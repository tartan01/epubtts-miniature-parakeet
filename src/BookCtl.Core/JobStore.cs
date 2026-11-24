using System.Globalization;
using Microsoft.Data.Sqlite;

namespace BookCtl.Core;

public sealed class JobStore
{
    private readonly string _db;
    private readonly object _gate = new();

    public string DbPath => _db;

    public JobStore(string dbPath)
    {
        _db = dbPath;
        var dir = Path.GetDirectoryName(dbPath);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
        Init();
    }

    private SqliteConnection Open()
    {
        var c = new SqliteConnection($"Data Source={_db};Cache=Shared");
        c.Open();
        return c;
    }

    private void Init()
    {
        using var c = Open();
        using var cmd = c.CreateCommand();
        cmd.CommandText = """
            PRAGMA journal_mode=WAL;
            PRAGMA busy_timeout=8000;
            CREATE TABLE IF NOT EXISTS jobs(
                job_id TEXT PRIMARY KEY,
                idempotency_key TEXT UNIQUE,
                input_path TEXT NOT NULL,
                book_id TEXT,
                book_title TEXT,
                status TEXT NOT NULL,
                phase TEXT,
                revision INTEGER NOT NULL DEFAULT 1,
                config_hash TEXT,
                profile_hash TEXT,
                args_hash TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                progress REAL NOT NULL DEFAULT 0,
                chapters_done INTEGER NOT NULL DEFAULT 0,
                chapters_total INTEGER NOT NULL DEFAULT 0,
                chars_done INTEGER NOT NULL DEFAULT 0,
                chars_total INTEGER NOT NULL DEFAULT 0,
                audio_seconds REAL NOT NULL DEFAULT 0,
                gpu_seconds REAL NOT NULL DEFAULT 0,
                error_code TEXT,
                error_message TEXT,
                result_path TEXT,
                cancel_requested INTEGER NOT NULL DEFAULT 0,
                engine TEXT,
                voice TEXT
            );
            CREATE TABLE IF NOT EXISTS chapters(
                chapter_id TEXT NOT NULL,
                job_id TEXT NOT NULL,
                idx INTEGER NOT NULL,
                level INTEGER NOT NULL DEFAULT 1,
                title TEXT,
                href TEXT,
                anchor TEXT,
                kind TEXT,
                chars INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'pending',
                audio_path TEXT,
                sha256 TEXT,
                seconds REAL NOT NULL DEFAULT 0,
                cer REAL,
                attempts INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY(job_id, chapter_id)
            );
            CREATE TABLE IF NOT EXISTS events(
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id TEXT NOT NULL,
                ts TEXT NOT NULL,
                level TEXT NOT NULL,
                stage TEXT NOT NULL,
                code TEXT NOT NULL,
                message TEXT NOT NULL,
                data_json TEXT
            );
            CREATE INDEX IF NOT EXISTS ix_events_job ON events(job_id, seq);
            CREATE INDEX IF NOT EXISTS ix_jobs_status ON jobs(status, created_at);
            """;
        cmd.ExecuteNonQuery();
    }

    public JobRecord InsertJob(JobRecord j, string argsHash)    {
        lock (_gate)
        {
            using var c = Open();
            j.ArgsHash = argsHash;
            using (var cmd = c.CreateCommand())
            {
                cmd.CommandText = """
                    INSERT INTO jobs(job_id,idempotency_key,input_path,book_id,book_title,status,phase,revision,
                        config_hash,profile_hash,args_hash,created_at,updated_at,progress,chapters_done,chapters_total,
                        chars_done,chars_total,audio_seconds,gpu_seconds,error_code,error_message,result_path,
                        cancel_requested,engine,voice)
                    VALUES(@job_id,@idk,@input,@book_id,@book_title,@status,@phase,1,
                        @cfg,@prof,@args,@ca,@ua,0,0,0,0,0,0,0,NULL,NULL,NULL,0,@eng,@vc)
                    """;
                P(cmd, "job_id", j.JobId);
                P(cmd, "idk", (object?)j.IdempotencyKey ?? DBNull.Value);
                P(cmd, "input", j.InputPath);
                P(cmd, "book_id", (object?)j.BookId ?? DBNull.Value);
                P(cmd, "book_title", (object?)j.BookTitle ?? DBNull.Value);
                P(cmd, "status", j.Status);
                P(cmd, "phase", (object?)j.Phase ?? DBNull.Value);
                P(cmd, "cfg", (object?)j.ConfigHash ?? DBNull.Value);
                P(cmd, "prof", (object?)j.ProfileHash ?? DBNull.Value);
                P(cmd, "args", argsHash);
                P(cmd, "ca", j.CreatedAt.ToString("o"));
                P(cmd, "ua", j.UpdatedAt.ToString("o"));
                P(cmd, "eng", (object?)j.Engine ?? DBNull.Value);
                P(cmd, "vc", (object?)j.Voice ?? DBNull.Value);
                try
                {
                    cmd.ExecuteNonQuery();
                    return j;
                }
                catch (SqliteException e) when (e.SqliteErrorCode == 19)
                {
                    var existing = FindByIdempotencyKey(j.IdempotencyKey);
                    if (existing is not null && existing.ArgsHash != argsHash)
                        throw new BookCtlException("IDEMPOTENCY_KEY_CONFLICT",
                            "该幂等键已绑定到不同参数的任务",
                            field: "idempotency_key", value: j.IdempotencyKey,
                            fix: "换用新的幂等键，或查询已存在的任务",
                            example: $"bookctl job status {existing.JobId}",
                            suggestions: [$"bookctl job status {existing.JobId}"]);
                    return existing!;
                }
            }
        }
    }

    public JobRecord? GetJob(string jobId)
    {
        using var c = Open();
        using var cmd = c.CreateCommand();
        cmd.CommandText = "SELECT * FROM jobs WHERE job_id=@id";
        P(cmd, "id", jobId);
        using var r = cmd.ExecuteReader();
        return r.Read() ? ReadJob(r) : null;
    }

    public JobRecord? FindByIdempotencyKey(string? key)
    {
        if (string.IsNullOrWhiteSpace(key)) return null;
        using var c = Open();
        using var cmd = c.CreateCommand();
        cmd.CommandText = "SELECT * FROM jobs WHERE idempotency_key=@k";
        P(cmd, "k", key);
        using var r = cmd.ExecuteReader();
        return r.Read() ? ReadJob(r) : null;
    }

    public List<JobRecord> ListJobs(string? status, int limit = 50)
    {
        using var c = Open();
        using var cmd = c.CreateCommand();
        cmd.CommandText = string.IsNullOrEmpty(status)
            ? "SELECT * FROM jobs ORDER BY created_at DESC LIMIT @lim"
            : "SELECT * FROM jobs WHERE status=@st ORDER BY created_at DESC LIMIT @lim";
        P(cmd, "st", status);
        P(cmd, "lim", limit);
        var list = new List<JobRecord>();
        using var r = cmd.ExecuteReader();
        while (r.Read()) list.Add(ReadJob(r));
        return list;
    }

    public bool UpdateJob(JobRecord j)
    {
        lock (_gate)
        {
            using var c = Open();
            j.UpdatedAt = DateTime.UtcNow;
            using var cmd = c.CreateCommand();
            cmd.CommandText = """
                UPDATE jobs SET status=@status, phase=@phase, book_id=@book_id, book_title=@book_title,
                    revision=revision+1, profile_hash=@prof,
                    updated_at=@ua, progress=@prog, chapters_done=@cd, chapters_total=@ct,
                    chars_done=@chd, chars_total=@cht, audio_seconds=@ascs, gpu_seconds=@gs,
                    error_code=@ec, error_message=@em, result_path=@res, cancel_requested=@cxl,
                    engine=@eng, voice=@vc
                WHERE job_id=@id AND revision=@rev
                """;
            P(cmd, "status", j.Status);
            P(cmd, "phase", (object?)j.Phase ?? DBNull.Value);
            P(cmd, "book_id", (object?)j.BookId ?? DBNull.Value);
            P(cmd, "book_title", (object?)j.BookTitle ?? DBNull.Value);
            P(cmd, "prof", (object?)j.ProfileHash ?? DBNull.Value);
            P(cmd, "ua", j.UpdatedAt.ToString("o"));
            P(cmd, "prog", j.Progress);
            P(cmd, "cd", j.ChaptersDone);
            P(cmd, "ct", j.ChaptersTotal);
            P(cmd, "chd", j.CharsDone);
            P(cmd, "cht", j.CharsTotal);
            P(cmd, "ascs", j.AudioSeconds);
            P(cmd, "gs", j.GpuSeconds);
            P(cmd, "ec", (object?)j.ErrorCode ?? DBNull.Value);
            P(cmd, "em", (object?)j.ErrorMessage ?? DBNull.Value);
            P(cmd, "res", (object?)j.ResultPath ?? DBNull.Value);
            P(cmd, "cxl", j.CancelRequested ? 1 : 0);
            P(cmd, "eng", (object?)j.Engine ?? DBNull.Value);
            P(cmd, "vc", (object?)j.Voice ?? DBNull.Value);
            P(cmd, "id", j.JobId);
            P(cmd, "rev", j.Revision);
            var rows = cmd.ExecuteNonQuery();
            if (rows > 0) j.Revision++;
            return rows > 0;
        }
    }

    public JobRecord? TryClaimQueued()
    {
        lock (_gate)
        {
            using var c = Open();
            using (var tx = c.BeginTransaction())
            {
                JobRecord? job = null;
                using (var q = c.CreateCommand())
                {
                    q.Transaction = tx;
                    q.CommandText = "SELECT * FROM jobs WHERE status='queued' ORDER BY created_at ASC LIMIT 1";
                    using var r = q.ExecuteReader();
                    if (r.Read()) job = ReadJob(r);
                }
                if (job is null) return null;
                using (var u = c.CreateCommand())
                {
                    u.Transaction = tx;
                    u.CommandText = "UPDATE jobs SET status='running', phase='S1_INGEST', updated_at=@ua, revision=revision+1 WHERE job_id=@id AND revision=@rev AND status='queued'";
                    P(u, "ua", DateTime.UtcNow.ToString("o"));
                    P(u, "id", job.JobId);
                    P(u, "rev", job.Revision);
                    if (u.ExecuteNonQuery() == 0) return null;
                }
                tx.Commit();
                job.Status = "running";
                job.Phase = "S1_INGEST";
                job.Revision++;
                return job;
            }
        }
    }

    public bool RequestCancel(string jobId)
    {
        using var c = Open();
        using var cmd = c.CreateCommand();
        cmd.CommandText = "UPDATE jobs SET cancel_requested=1, updated_at=@ua WHERE job_id=@id";
        P(cmd, "ua", DateTime.UtcNow.ToString("o"));
        P(cmd, "id", jobId);
        return cmd.ExecuteNonQuery() > 0;
    }

    public bool IsCancelRequested(string jobId)
    {
        using var c = Open();
        using var cmd = c.CreateCommand();
        cmd.CommandText = "SELECT cancel_requested FROM jobs WHERE job_id=@id";
        P(cmd, "id", jobId);
        return cmd.ExecuteScalar() is 1L;
    }

    public void ResetForResume(string jobId)
    {
        using var c = Open();
        using var cmd = c.CreateCommand();
        cmd.CommandText = "UPDATE jobs SET status='queued', phase=NULL, cancel_requested=0, updated_at=@ua, revision=revision+1 WHERE job_id=@id AND status IN ('failed','cancelled')";
        P(cmd, "ua", DateTime.UtcNow.ToString("o"));
        P(cmd, "id", jobId);
        if (cmd.ExecuteNonQuery() == 0)
            throw new BookCtlException("INVALID_STATE_TRANSITION", "仅 failed / cancelled 的任务可以恢复",
                fix: "确认任务状态后重试", example: "bookctl job status " + jobId);
    }

    public void InsertChapters(string jobId, List<Chapter> chapters)
    {
        using var c = Open();
        using (var del = c.CreateCommand())
        {
            del.CommandText = "DELETE FROM chapters WHERE job_id=@id";
            P(del, "id", jobId);
            del.ExecuteNonQuery();
        }
        using var tx = c.BeginTransaction();
        foreach (var ch in chapters)
        {
            using var cmd = c.CreateCommand();
            cmd.Transaction = tx;
            cmd.CommandText = """
                INSERT INTO chapters(chapter_id,job_id,idx,level,title,href,anchor,kind,chars,status,
                    audio_path,sha256,seconds,cer,attempts)
                VALUES(@cid,@jid,@idx,@lvl,@title,@href,@anchor,@kind,@chars,@status,@ap,@sha,@sec,@cer,@att)
                """;
            P(cmd, "cid", ch.Id);
            P(cmd, "jid", jobId);
            P(cmd, "idx", ch.Index);
            P(cmd, "lvl", ch.Level);
            P(cmd, "title", ch.Title);
            P(cmd, "href", ch.Href);
            P(cmd, "anchor", (object?)ch.Anchor ?? DBNull.Value);
            P(cmd, "kind", ch.Kind);
            P(cmd, "chars", ch.CharCount);
            P(cmd, "status", ch.Status);
            P(cmd, "ap", (object?)ch.AudioPath ?? DBNull.Value);
            P(cmd, "sha", (object?)ch.Sha256 ?? DBNull.Value);
            P(cmd, "sec", ch.Seconds);
            P(cmd, "cer", (object?)ch.Cer ?? DBNull.Value);
            P(cmd, "att", ch.Attempts);
            cmd.ExecuteNonQuery();
        }
        tx.Commit();
    }

    public List<Chapter> GetChapters(string jobId)
    {
        using var c = Open();
        using var cmd = c.CreateCommand();
        cmd.CommandText = "SELECT * FROM chapters WHERE job_id=@id ORDER BY idx";
        P(cmd, "id", jobId);
        var list = new List<Chapter>();
        using var r = cmd.ExecuteReader();
        while (r.Read())
        {
            list.Add(new Chapter
            {
                Id = r.GetString(r.GetOrdinal("chapter_id")),
                Index = (int)r.GetInt64(r.GetOrdinal("idx")),
                Level = (int)r.GetInt64(r.GetOrdinal("level")),
                Title = r.GetString(r.GetOrdinal("title")),
                Href = r.GetString(r.GetOrdinal("href")),
                Anchor = r.IsDBNull(r.GetOrdinal("anchor")) ? null : r.GetString(r.GetOrdinal("anchor")),
                Kind = r.GetString(r.GetOrdinal("kind")),
                CharCount = (int)r.GetInt64(r.GetOrdinal("chars")),
                Status = r.GetString(r.GetOrdinal("status")),
                AudioPath = r.IsDBNull(r.GetOrdinal("audio_path")) ? null : r.GetString(r.GetOrdinal("audio_path")),
                Sha256 = r.IsDBNull(r.GetOrdinal("sha256")) ? null : r.GetString(r.GetOrdinal("sha256")),
                Seconds = r.GetDouble(r.GetOrdinal("seconds")),
                Cer = r.IsDBNull(r.GetOrdinal("cer")) ? null : r.GetDouble(r.GetOrdinal("cer")),
                Attempts = (int)r.GetInt64(r.GetOrdinal("attempts")),
            });
        }
        return list;
    }

    public void UpdateChapter(string jobId, Chapter ch)
    {
        using var c = Open();
        using var cmd = c.CreateCommand();
        cmd.CommandText = """
            UPDATE chapters SET status=@status, audio_path=@ap, sha256=@sha, seconds=@sec, cer=@cer, attempts=@att
            WHERE job_id=@jid AND chapter_id=@cid
            """;
        P(cmd, "status", ch.Status);
        P(cmd, "ap", (object?)ch.AudioPath ?? DBNull.Value);
        P(cmd, "sha", (object?)ch.Sha256 ?? DBNull.Value);
        P(cmd, "sec", ch.Seconds);
        P(cmd, "cer", (object?)ch.Cer ?? DBNull.Value);
        P(cmd, "att", ch.Attempts);
        P(cmd, "jid", jobId);
        P(cmd, "cid", ch.Id);
        cmd.ExecuteNonQuery();
    }

    public long AppendEvent(JobEvent e)
    {
        lock (_gate)
        {
            using var c = Open();
            using var cmd = c.CreateCommand();
            cmd.CommandText = """
                INSERT INTO events(job_id,ts,level,stage,code,message,data_json)
                VALUES(@jid,@ts,@lvl,@stage,@code,@msg,@data);
                SELECT last_insert_rowid();
                """;
            P(cmd, "jid", e.JobId);
            P(cmd, "ts", e.Ts);
            P(cmd, "lvl", e.Level);
            P(cmd, "stage", e.Stage);
            P(cmd, "code", e.Code);
            P(cmd, "msg", e.Message);
            P(cmd, "data", (object?)e.DataJson ?? DBNull.Value);
            return (long)(cmd.ExecuteScalar() ?? 0L);
        }
    }

    public List<JobEvent> ReadEvents(string jobId, long sinceSeq, int limit)
    {
        using var c = Open();
        using var cmd = c.CreateCommand();
        cmd.CommandText = "SELECT * FROM events WHERE job_id=@jid AND seq>@since ORDER BY seq ASC LIMIT @lim";
        P(cmd, "jid", jobId);
        P(cmd, "since", sinceSeq);
        P(cmd, "lim", limit);
        var list = new List<JobEvent>();
        using var r = cmd.ExecuteReader();
        while (r.Read())
        {
            list.Add(new JobEvent
            {
                Seq = r.GetInt64(r.GetOrdinal("seq")),
                JobId = r.GetString(r.GetOrdinal("job_id")),
                Ts = r.GetString(r.GetOrdinal("ts")),
                Level = r.GetString(r.GetOrdinal("level")),
                Stage = r.GetString(r.GetOrdinal("stage")),
                Code = r.GetString(r.GetOrdinal("code")),
                Message = r.GetString(r.GetOrdinal("message")),
                DataJson = r.IsDBNull(r.GetOrdinal("data_json")) ? null : r.GetString(r.GetOrdinal("data_json")),
            });
        }
        return list;
    }

    public long MaxEventSeq(string jobId)
    {
        using var c = Open();
        using var cmd = c.CreateCommand();
        cmd.CommandText = "SELECT COALESCE(MAX(seq),0) FROM events WHERE job_id=@jid";
        P(cmd, "jid", jobId);
        return (long)(cmd.ExecuteScalar() ?? 0L);
    }

    private static JobRecord ReadJob(SqliteDataReader r) => new()
    {
        JobId = r.GetString(r.GetOrdinal("job_id")),
        IdempotencyKey = r.IsDBNull(r.GetOrdinal("idempotency_key")) ? null : r.GetString(r.GetOrdinal("idempotency_key")),
        InputPath = r.GetString(r.GetOrdinal("input_path")),
        BookId = r.IsDBNull(r.GetOrdinal("book_id")) ? null : r.GetString(r.GetOrdinal("book_id")),
        BookTitle = r.IsDBNull(r.GetOrdinal("book_title")) ? null : r.GetString(r.GetOrdinal("book_title")),
        Status = r.GetString(r.GetOrdinal("status")),
        Phase = r.IsDBNull(r.GetOrdinal("phase")) ? null : r.GetString(r.GetOrdinal("phase")),
        Revision = r.GetInt64(r.GetOrdinal("revision")),
        ConfigHash = r.IsDBNull(r.GetOrdinal("config_hash")) ? null : r.GetString(r.GetOrdinal("config_hash")),
        ProfileHash = r.IsDBNull(r.GetOrdinal("profile_hash")) ? null : r.GetString(r.GetOrdinal("profile_hash")),
        ArgsHash = r.IsDBNull(r.GetOrdinal("args_hash")) ? null : r.GetString(r.GetOrdinal("args_hash")),
        CreatedAt = DateTime.Parse(r.GetString(r.GetOrdinal("created_at")), null, DateTimeStyles.RoundtripKind),
        UpdatedAt = DateTime.Parse(r.GetString(r.GetOrdinal("updated_at")), null, DateTimeStyles.RoundtripKind),
        Progress = r.GetDouble(r.GetOrdinal("progress")),
        ChaptersDone = (int)r.GetInt64(r.GetOrdinal("chapters_done")),
        ChaptersTotal = (int)r.GetInt64(r.GetOrdinal("chapters_total")),
        CharsDone = (int)r.GetInt64(r.GetOrdinal("chars_done")),
        CharsTotal = (int)r.GetInt64(r.GetOrdinal("chars_total")),
        AudioSeconds = r.GetDouble(r.GetOrdinal("audio_seconds")),
        GpuSeconds = r.GetDouble(r.GetOrdinal("gpu_seconds")),
        ErrorCode = r.IsDBNull(r.GetOrdinal("error_code")) ? null : r.GetString(r.GetOrdinal("error_code")),
        ErrorMessage = r.IsDBNull(r.GetOrdinal("error_message")) ? null : r.GetString(r.GetOrdinal("error_message")),
        ResultPath = r.IsDBNull(r.GetOrdinal("result_path")) ? null : r.GetString(r.GetOrdinal("result_path")),
        CancelRequested = r.GetInt64(r.GetOrdinal("cancel_requested")) == 1,
        Engine = r.IsDBNull(r.GetOrdinal("engine")) ? null : r.GetString(r.GetOrdinal("engine")),
        Voice = r.IsDBNull(r.GetOrdinal("voice")) ? null : r.GetString(r.GetOrdinal("voice")),
    };

    private static void P(SqliteCommand cmd, string name, object? value)
    {
        cmd.Parameters.AddWithValue("@" + name, value);
    }

    // ——— 孤儿任务回收：Worker 进程被强杀 / 崩溃后，jobs.status 会永久停在 running ———

    public List<string> FindRunningJobIds()
    {
        lock (_gate)
        {
            using var c = Open();
            using var cmd = c.CreateCommand();
            cmd.CommandText = "SELECT job_id FROM jobs WHERE status='running'";
            using var r = cmd.ExecuteReader();
            var list = new List<string>();
            while (r.Read()) list.Add(r.GetString(0));
            return list;
        }
    }

    /// <summary>仅当任务仍是 running 时才标记失败（CAS），避免误伤已被重新领取的任务。</summary>
    public bool FailIfStillRunning(string jobId, string code, string message)
    {
        lock (_gate)
        {
            using var c = Open();
            using var cmd = c.CreateCommand();
            cmd.CommandText = """
                UPDATE jobs SET status='failed', error_code=@code, error_message=@msg,
                    revision=revision+1, updated_at=@ua
                WHERE job_id=@id AND status='running'
                """;
            P(cmd, "code", code);
            P(cmd, "msg", message);
            P(cmd, "ua", DateTime.UtcNow.ToString("o", CultureInfo.InvariantCulture));
            P(cmd, "id", jobId);
            return cmd.ExecuteNonQuery() > 0;
        }
    }
}
