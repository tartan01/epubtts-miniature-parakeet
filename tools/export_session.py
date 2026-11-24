#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
把 WorkBuddy 会话转录（.jsonl）导出为可入库的 Markdown 交接文档。

用法：
    python tools/export_session.py <session.jsonl> <docs/conversation 输出目录>

产出：
    01-full-dialogue.md      完整用户/助手对话文本（按时间顺序）
    02-context-summaries.md  上下文压缩点的历史摘要（去重后的历史对话）
    03-action-ledger.md      全量工具调用台账（一行一次调用）

设计原则：
  - 只导出「对话内容」：用户实际输入、助手文本回复、历史摘要、工具动作。
  - 不导出宿主注入的 system-reminder / identity_context / project_context、
    模型内部 reasoning、以及工具结果里的原始大块输出（这些属于运行时内部信息）。
  - 原始 jsonl 保留在本机，不入代码仓库。
"""
from __future__ import annotations

import json
import re
import sys
from collections import Counter
from datetime import datetime, timedelta, timezone
from pathlib import Path

TZ = timezone(timedelta(hours=8))


def stamp(row: dict) -> str:
    ts = row.get("timestamp")
    if not isinstance(ts, (int, float)):
        return "unknown-time"
    return datetime.fromtimestamp(ts / 1000, TZ).strftime("%Y-%m-%d %H:%M:%S")


def message_text(row: dict) -> str:
    content = row.get("content")
    if isinstance(content, list):
        return "".join(
            part.get("text", "")
            for part in content
            if isinstance(part, dict)
            and part.get("type") in ("input_text", "output_text", "text")
        )
    return str(content or "")


def clean_user_query(text: str) -> str | None:
    m = re.search(r"<user_query>(.*?)</user_query>", text, re.S)
    if not m:
        return None
    return m.group(1).strip()


def task_notification(text: str) -> str | None:
    if "<task-notification>" not in text:
        return None
    tid = re.search(r"<task-id>(.*?)</task-id>", text, re.S)
    summary = re.search(r"<summary>(.*?)</summary>", text, re.S)
    raw = re.sub(r"<[^>]+>", "", summary.group(1)) if summary else ""
    raw = re.sub(r"\s+", " ", raw).strip()
    return f"后台任务 `{tid.group(1).strip() if tid else '?'}` 通知：{raw[:220]}"


SUMMARY_TAG = ("<cb_summary", "<conversation_history_summary")


def clean_summary(text: str) -> str:
    """去掉工具调用/结果块，把 remaining XML 标签转成 Markdown 结构。"""
    s = re.sub(r"<previous_tool_call\b.*?</previous_tool_call>", "", text, flags=re.S)
    s = re.sub(r"<tool_use_id>.*?</tool_use_id>", "", s, flags=re.S)
    s = re.sub(r"<result>.*?</result>", "", s, flags=re.S)
    s = re.sub(r"<omitted\s*/>", "", s)
    s = re.sub(r"<previous_user_message>", "\n\n**用户（历史）**\n\n> ", s)
    s = re.sub(r"</previous_user_message>", "\n", s)
    s = re.sub(r"<previous_assistant_message>", "\n\n**助手（历史）**\n\n", s)
    s = re.sub(r"</previous_assistant_message>", "\n", s)
    s = re.sub(r"<(cb_summary|conversation_history_summary)[^>]*>|</(cb_summary|conversation_history_summary)>", "", s)
    s = re.sub(r"<system-reminder[^>]*>.*?</system-reminder>", "", s, flags=re.S)
    s = re.sub(r"\n{4,}", "\n\n\n", s)
    return s.strip()


def arg_brief(name: str, arguments) -> str:
    if isinstance(arguments, str):
        try:
            arguments = json.loads(arguments)
        except Exception:
            arguments = {"raw": arguments}
    if not isinstance(arguments, dict):
        return ""
    keys = {
        "Bash": ("command", "description"),
        "PowerShell": ("command", "description"),
        "Read": ("file_path",),
        "Write": ("file_path", "content"),
        "Edit": ("file_path", "old_string", "new_string"),
        "MultiEdit": ("file_path", "edits"),
        "Grep": ("pattern", "path"),
        "Glob": ("pattern",),
        "WebSearch": ("query",),
        "WebFetch": ("url",),
        "TaskCreate": ("subject",),
        "TaskUpdate": ("taskId", "status"),
        "Agent": ("description", "subagent_type"),
        "ViewImage": ("image_path",),
        "present_files": ("files",),
        "Skill": ("skill",),
        "TaskOutput": ("task_id",),
    }.get(name, ())
    parts = []
    for k in keys:
        v = arguments.get(k)
        if v is None:
            continue
        v = str(v).replace("\r", " ").replace("\n", " ")
        v = re.sub(r"\s+", " ", v).strip()
        if len(v) > 150:
            v = v[:150] + "…"
        parts.append(f"{k}=`{v}`")
    if not parts:
        for k, v in list(arguments.items())[:2]:
            v = re.sub(r"\s+", " ", str(v)).strip()
            parts.append(f"{k}=`{v[:110] + ('…' if len(v) > 110 else '')}`")
    return " · ".join(parts)


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    src, outdir = Path(sys.argv[1]), Path(sys.argv[2])
    outdir.mkdir(parents=True, exist_ok=True)
    rows = [json.loads(line) for line in src.open(encoding="utf-8") if line.strip()]

    dialogue: list[str] = []
    summaries: list[str] = []
    ledger: list[str] = []

    dialogue.append(
        "# 本期完整对话记录\n\n"
        "> 由 `tools/export_session.py` 从 WorkBuddy 会话转录自动导出。\n"
        "> 时间均为 GMT+8。\n"
        "> 清洗规则：只保留用户实际输入（`<user_query>` 正文）、助手文本回复、"
        "后台任务通知摘要；宿主注入的运行时上下文（system-reminder / identity / project context）、"
        "模型内部 reasoning、工具结果的原始大块输出不属于「对话内容」，未导出。\n"
        "> 上下文压缩点以 `📌` 标注，对应历史全文见 `02-context-summaries.md`。\n"
        "> 所有工具动作见 `03-action-ledger.md`。\n\n---\n"
    )
    summaries.append(
        "# 上下文压缩历史摘要\n\n"
        "> 本期会话经历 3 次上下文压缩。以下为压缩时保留的历史对话与结论，"
        "用于补齐 `01-full-dialogue.md` 中被折叠的早期内容。\n"
        "> 已剔除工具调用块与 `<omitted />` 占位；重复内容按出现顺序保留。\n\n---\n"
    )
    ledger.append(
        "# 工具动作台账（本期全量）\n\n"
        "> 按时间顺序逐条列出 Agent 的每一次工具调用（共 "
        f"{sum(1 for r in rows if r.get('type') == 'function_call')} 次）。\n"
        "> 参数为摘要（超长截断），结果原文未导出；需要细节请查本机原始 jsonl。\n\n---\n"
    )

    n_user = n_assistant = n_summary = n_event = 0
    calls = Counter()
    for row in rows:
        rtype = row.get("type")
        ts = stamp(row)

        if rtype == "message":
            text = message_text(row)
            role = row.get("role")
            if role == "user":
                # 顺序很重要：压缩摘要的正文里含有历史 <user_query> 与
                # <task-notification> 片段，必须先于二者判定，否则整段摘要会被
                # 误当成一条用户输入或一条后台任务通知。
                if any(t in text for t in SUMMARY_TAG):
                    n_summary += 1
                    dialogue.append(
                        f"\n> **📌 上下文压缩点 C{n_summary} · {ts}**："
                        f"此处折叠了早期会话，摘要全文见 `02-context-summaries.md` §C{n_summary}。\n"
                    )
                    summaries.append(f"\n## C{n_summary} · {ts}\n\n{clean_summary(text)}\n")
                    continue
                note = task_notification(text)
                if note:
                    n_event += 1
                    dialogue.append(f"\n> **[系统事件 · {ts}]** {note}\n")
                    continue
                q = clean_user_query(text)
                if q is not None:
                    n_user += 1
                    dialogue.append(f"\n## 🧑 用户 · {ts} · U{n_user}\n\n{q}\n")
                    continue
                # 无标签的续接指令等
                n_user += 1
                plain = re.sub(r"<[^>]+>", "", text).strip()
                if plain:
                    dialogue.append(f"\n## 🧑 用户（续接指令） · {ts} · U{n_user}\n\n{plain}\n")
            elif role == "assistant":
                n_assistant += 1
                dialogue.append(f"\n### 🤖 助手 · {ts} · A{n_assistant}\n\n{text.strip()}\n")

        elif rtype == "function_call":
            name = row.get("name") or "(unknown)"
            calls[name] += 1
            ledger.append(f"- `{ts}` **{name}** · {arg_brief(name, row.get('arguments')) or '—'}")

    ledger.append(
        "\n\n---\n\n## 工具调用统计\n\n| 工具 | 次数 |\n|---|---:|\n"
        + "\n".join(f"| `{k}` | {v} |" for k, v in calls.most_common())
        + f"\n\n合计 **{sum(calls.values())}** 次。\n"
    )

    (outdir / "01-full-dialogue.md").write_text("".join(dialogue), encoding="utf-8")
    (outdir / "02-context-summaries.md").write_text("".join(summaries), encoding="utf-8")
    (outdir / "03-action-ledger.md").write_text("\n".join(ledger) + "\n", encoding="utf-8")

    print(f"user_messages={n_user} assistant_messages={n_assistant} "
          f"summaries={n_summary} system_events={n_event} tool_calls={sum(calls.values())}")
    for p in sorted(outdir.glob("*.md")):
        print(f"  {p.name}: {p.stat().st_size} bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
