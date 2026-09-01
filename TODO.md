Tauri 项目的版本通常位于：

```text
src-tauri/tauri.conf.json
```

例如：

```json
{
  "version": "0.1.0"
}
```

每次发布前将它改成新的版本，例如：

```json
{
  "version": "0.1.1"
}
```

当前 workflow 会据此生成：

```text
Tag: app-v0.1.1
Release: yssbi v0.1.1
```

建议遵循语义化版本：

- `0.1.1`：Bug 修复
- `0.2.0`：新增兼容功能
- `1.0.0`：重大或稳定版本
- `2.0.0`：不兼容的重大更新
