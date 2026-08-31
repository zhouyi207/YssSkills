# 功能开发流程

本清单用于跨前端、Tauri boundary 和 Rust crate 的功能变更；命令细节与验证矩阵见
[`LOCAL_WORKFLOW.md`](LOCAL_WORKFLOW.md)，架构边界见
[`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.md)。

1. 明确用户可观察行为、失败语义、数据事实源和完成证据。
2. 按现有责任边界选择 owner；Tauri command 只做 transport，前端 view 只做展示与交互。
3. 为变化的项目自有契约添加最小回归测试，优先通过 public seam 验证。
4. 实现时保持 typed error、显式 IPC DTO、受控路径和资源生命周期，不扩大无关范围。
5. 先运行受影响的聚焦检查，再按改动范围运行完整门禁与 `git diff --check`。
6. 同一变更中更新失真的维护型文档，提交前检查临时文件、生成物、凭据和无关 diff。
