# Windows 翻译失败路径续作（2026-09-08）

起点 `389fbe5`，对应 UI-03/UI-04、V09 的流式失败反馈与请求隔离。本轮不重新接管此前由用户 Esc 停止的桌面。

## DeepSeek 提前结束不能当作成功

源码核对发现，原实现虽然识别 `[DONE]`，但在未收到结束标记、连接正常 EOF 时，仍会返回已累积的非空文本；同时忽略 `finish_reason` 的异常值。

[DeepSeek Chat Completions 官方协议](https://api-docs.deepseek.com/api/create-chat-completion/) 定义流以 `data: [DONE]` 结束，`length`、`content_filter`、`insufficient_system_resource` 等表示截断或中断。

修正后使用增量解析器：按网络分片组装完整 UTF-8 行，收到结束标记才完成；提前 EOF、异常 finish_reason、服务端错误和空结果返回错误。最后一行没有换行符的 `[DONE]` 仍可识别，结束标记后的内容不再追加。现有原生错误路径可保留已显示的片段并呈现错误，不再把半截译文标为成功。

解析器逐行消费分片，移除了每行都从整个缓冲区头部 drain 的重复移动；单事件限 1 MiB，累积译文限 16 MiB，超过限制明确报错，避免缺少换行的流持续增长。

验证：`cargo test -p rotor-translator --lib --offline` 的 15 项测试通过，新增测试覆盖每个 UTF-8 分割位置、提前 EOF、异常终止、服务端错误、结束后的多余内容、无尾部换行、空结果及大小限制。包含依赖的 translator/ui/desktop `--all-targets --offline -- -D warnings` clippy 通过。

没有调用真实付费 API，也没有据单测关闭三引擎 GUI、IME 或 Windows 完整验收。
