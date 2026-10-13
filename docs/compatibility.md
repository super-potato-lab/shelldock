# FinalShell 连接兼容

## 已实现的输入

- 一个 `*_connect_config.json`：JSON 对象。
- 多个 JSON 文件；JSON 对象数组。
- 包含 `connections` 数组的配置包。
- FinalShell `conn/` 目录，或包含 `conn/` 的数据根目录。
- ShellDock `format: "shelldock"`、版本 1 的导出包。

兼容公开迁移工具中确认的字段：`name`、`host`、`port`、`user_name`、`authentication_type`、`conection_type`（原格式的拼写）、`parent_id`、`proxy_id`、`secret_key_id`、`port_forwarding_list`。

当前 `conection_type: 100` 按 SSH 处理，其他类型保留并提示不支持；`authentication_type: 1/2` 对应常见密码/私钥配置。不同版本的新增枚举需要真实文件验证，不能自行猜测。

`folder.json` 用 `id`/`parent_id` 构成分组树，处理缺失引用和循环引用。未知非敏感字段保留在 `extra`，不会被默默丢弃。未迁移的代理引用会阻止连接，必须显式清除该引用后使用直接连接。

## 密码

支持公开的 FinalShell 旧版 Base64 + 8 字节头 + DES/ECB/PKCS5 envelope。Rust 实现对照了独立的非敏感向量及 Java Random 参考值。

这仅用于迁移兼容，ShellDock 不使用 DES 保存自己的密码。导入后的密码进入系统凭据服务；不可用时存于本次进程内并显示提示。

无法识别的密码格式会明确提示重新输入，不会把未知密文当成明文发送。

ShellDock 原生导出始终不含密码；FinalShell 兼容导出默认不含密码，勾选“包含已保存的密码”后才生成旧格式密文。旧格式可逆，导出文件应由用户自行保管。

## 私钥、代理、转发

单连接文件中的 `secret_key_id` 并不包含私钥本身。当前不从全局 `secret_key_list` 自动导出密钥材料；导入后重新选本机私钥路径。

代理与端口转发字段保留，但执行尚未实现。不会宣称它们已迁移为可用功能。

## 导出

- ShellDock JSON：可导出全部或一个连接，保留应用分组。
- FinalShell 单连接 JSON：使用原字段名，导出一个选中连接；原始未知字段保留，`parent_id` 设为 `root`，另保留 `group_path`。
- FinalShell 接收方是否读取 `group_path` 尚未验证，因此单连接导出不承诺恢复接收方原分组树。

## 限制与验收

每文件最多 8 MiB，最多 5000 项，目录递归最多 32 层，跳过符号链接。加密备份 ZIP、未知格式、引用的全局代理/密钥尚不支持。

真实验收需要来自用户使用版本的导出样本，以及一个用户选择的测试主机。不要把真实连接文件提交到仓库。格式样本可替换主机与用户名，但若验证密码迁移，应使用专门创建的虚拟测试连接。
