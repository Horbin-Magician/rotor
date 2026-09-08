# Windows 翻译失败路径续作（2026-09-08）

起点 `389fbe5`，对应 UI-03/UI-04、V09 的流式失败反馈与请求隔离。本轮不重新接管此前由用户 Esc 停止的桌面。

## DeepSeek 提前结束不能当作成功

源码核对发现，原实现虽然识别 `[DONE]`，但在未收到结束标记、连接正常 EOF 时，仍会返回已累积的非空文本；同时忽略 `finish_reason` 的异常值。

[DeepSeek Chat Completions 官方协议](https://api-docs.deepseek.com/api/create-chat-completion/) 定义流以 `data: [DONE]` 结束，`length`、`content_filter`、`insufficient_system_resource` 等表示截断或中断。

修正后使用增量解析器：按网络分片组装完整 UTF-8 行，收到结束标记才完成；提前 EOF、异常 finish_reason、服务端错误和空结果返回错误。最后一行没有换行符的 `[DONE]` 仍可识别，结束标记后的内容不再追加。现有原生错误路径可保留已显示的片段并呈现错误，不再把半截译文标为成功。

解析器逐行消费分片，移除了每行都从整个缓冲区头部 drain 的重复移动；单事件限 1 MiB，累积译文限 16 MiB，超过限制明确报错，避免缺少换行的流持续增长。

验证：`cargo test -p rotor-translator --lib --offline` 的 15 项测试通过，新增测试覆盖每个 UTF-8 分割位置、提前 EOF、异常终止、服务端错误、结束后的多余内容、无尾部换行、空结果及大小限制。包含依赖的 translator/ui/desktop `--all-targets --offline -- -D warnings` clippy 通过。

没有调用真实付费 API，也没有据单测关闭三引擎 GUI、IME 或 Windows 完整验收。

## 实际 HTTP 请求的替换与取消

在 `rotor-runtime` 增加两项 localhost 集成测试，使用临时 profile、真实 reqwest 请求及实际 runtime 事件队列；HTTP 响应通过通道控制释放顺序，而非依赖固定延时猜测请求先后。

- 先收到旧请求，再提交新请求；在新请求已到服务器后取消旧 ID，并依次释放旧/新响应，验证只收到新 ID 的正确译文。
- 再取消当前请求并释放其响应，验证取消后的结果不再投递。
- 服务返回 HTTP 503，验证得到带原请求 ID 的错误，而不是成功译文。

`cargo test -p rotor-runtime translation_tests --offline` 两项通过；runtime `--all-targets --offline -- -D warnings` clippy 与全 workspace 格式检查通过。该测试不注册系统热键、不创建窗口、不调用剪贴板或外部翻译服务。
