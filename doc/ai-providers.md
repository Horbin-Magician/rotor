# AI 服务商

在设置侧栏的「基础」下方打开「AI 服务商」，选择当前服务商，填写 API 密钥和模型 ID。各服务商分别保存基础地址、密钥、模型和最大输出 Token 数，切换不会覆盖其他服务商的配置。OpenAI、Claude 和自定义服务的模型 ID 需要填写当前账户可用的模型。

填写后可点击「测试配置」，使用当前输入值发送一个简短请求，验证地址、鉴权、模型访问和流式响应。页面显示成功或失败原因；修改配置或切换服务商后，旧结果会清除。测试会产生少量 API 用量。

在「翻译」中选择「AI 翻译」后，输入翻译和划词翻译共用当前全局服务商。Google 翻译不需要 AI 配置。

| 服务商 | 默认基础地址 | 协议 |
| --- | --- | --- |
| DeepSeek | `https://api.deepseek.com` | OpenAI Chat Completions 兼容 |
| OpenAI | `https://api.openai.com/v1` | OpenAI Chat Completions |
| Claude (Anthropic) | `https://api.anthropic.com/v1` | Anthropic Messages |
| 自定义 | 手动填写 | OpenAI 兼容或 Anthropic Messages |

基础地址需包含服务要求的版本路径，例如 `http://localhost:1234/v1`。也可以填写以 `/chat/completions` 或 `/messages` 结尾的完整接口地址。自定义服务允许空密钥，可用于无需鉴权的本地服务。地址不能包含用户名、密码、查询参数或 fragment；API 密钥通过请求头发送。接口地址不自动跟随重定向。

最大输出 Token 数默认 4096，应根据所用模型的限制调整。达到输出上限、服务返回错误或流意外中断时，翻译会报告失败，不会把不完整输出当作成功结果。

已有原生版本的 DeepSeek 配置会作为全局 DeepSeek 设置的默认值；显式保存的新值（包括清空密钥）优先。原配置及未知字段保留。已有自定义 URL 模板引擎继续运行，并显示旧配置提示；选择「AI 翻译」后改用全局服务。旧 URL 模板不是 AI 协议，不能直接填入 AI 基础地址。

接口实现依据 [OpenAI Chat API](https://developers.openai.com/api/reference/resources/chat) 和 [Anthropic 流式 Messages API](https://platform.claude.com/docs/en/build-with-claude/streaming)。自定义服务需实现所选协议；此设置不支持任意 HTTP 模板。

## 搜索中的 AI 对话

打开搜索框后按 **Tab** 进入 AI 模式，输入问题并按 **Enter** 发送；再次按 Tab 返回文件搜索。当前查询完成且没有结果时，按 Enter 会自动切换到 AI 模式并发送该查询。查询尚未完成时会等待结果；空输入和搜索服务错误不会自动发送。

对话使用此页面配置的 AI 提供商，与翻译请求相互独立。回复以 Markdown 流式显示，支持连续追问、复制、停止生成、失败重试和新对话。向上滚动阅读历史时不会被新内容拉回底部。切换回搜索或关闭窗口会停止正在生成的回复；关闭窗口后不保存聊天记录。中文输入法正在组词时，Enter 和 Tab 仍由输入法处理。
