# Windows 搜索关闭释放核验（2026-09-08）

对应 F04 / V08 / V21；源码核对起点 `40907ec`。

旧 `src/pages/Searcher.vue::hideWindow` 在隐藏后调用 `releaseSearch`。原生 SearchView 在失焦、Escape 或打开文件成功时关闭窗口，但主壳的窗口关闭协调遗漏了 `Services::release_search`。

本批在关闭协调中补回释放：先用实际关闭的 WindowId 找到当前注册角色，再移除该窗口并发送 Release。若旧窗口已被新窗口替换，旧 ID 不再匹配注册项，不会误释放新窗口正在使用的索引。释放请求与后续唤起 Update 都从同一 UI 线程按序投递。

同时发现共享 `FileData::release_index` 只清空查询名称，保留了已经完成的查询结果和 Vec 容量。新增回归测试在修复前实际失败于 `items.is_empty()`；修复后释放结果字符串、Vec 分配及分页状态，即使没有可用卷也先清理缓存。

验证：18 项 `cargo test -p rotor-searcher --lib --offline` 通过；新增测试核对结果为空、容量归零、查询名/分页复位，以及同一查询重新开始时不追加旧结果。native check、desktop/searcher 严格 clippy 与 workspace 格式检查通过。

没有操作真实用户索引或接管桌面。单测证明缓存所有权释放，不代表进程 RSS/GPU 内存曲线、100 次窗口循环或 UI 焦点验收已通过。
