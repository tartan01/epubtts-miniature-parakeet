using BookCtl.Core;

Console.OutputEncoding = System.Text.Encoding.UTF8;
var handlers = new BookCtlHandlers();
handlers.Worker.Start();
Console.Error.WriteLine($"[bookctl-worker] 已启动，工作区：{handlers.Config.EffectiveWorkspaceRoot}");
Console.Error.WriteLine($"[bookctl-worker] 数据库：{handlers.Config.DbPath}");
Console.Error.WriteLine("[bookctl-worker] Ctrl+C 退出（正在执行的任务保留章节级 checkpoint）");

var done = new ManualResetEventSlim(false);
Console.CancelKeyPress += (_, e) => { e.Cancel = true; done.Set(); };
done.Wait();

handlers.Worker.StopAsync().GetAwaiter().GetResult();
Console.Error.WriteLine("[bookctl-worker] 已停止");
