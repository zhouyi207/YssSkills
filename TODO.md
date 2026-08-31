1. skill-core：模型、解析、错误、纯规则和测试
2. skill-harness：内置 Harness 与自定义 adapter
3. skill-local：只读扫描、读取和 hash
4. skill-local：复制、跨平台 Link、删除和 watcher
5. skill-workspace：Global / Project / Linked 部署状态
6. skill-registry：skills.sh、GitHub 等远程来源
7. [x] SQLite 持久化 adapter
8. [x] Tauri commands、IPC DTO 和前端 service

C:\Users\Administrator\Downloads\skills-manager-main\skills-manager-main\

## 2026.08.29

1. [x] 在这里对 skills 进行索引的时候，需要读取home 目录下的 .agents/.skill-lock.json 文件中的skills信息，并将其信息于目前 skills 中存在的信息进行填充或者更新，没有就空着；将其中的 source 字段用来替换前端副标题显示的 Local · C:\Users\Administrator\.yss-skills\skills\brainstorming 信息；
2. [x] 在这里 registry 页面首页初始化后内容无法加载内容，请分析 C:\Users\Administrator\Downloads\skills-manager-main\skills-manager-main\ 项目的请求，在这里我认为可以打开yssSkills 项目后自动在后台静默加载；
3. [x] 处理完这两者后，需要修复 skills 中的 set，这里的 skills set 表示的意思是可以组合skills set，方便定义一个 skills 集合方便加载和取消，请完成这一步逻辑，这里的 modal 窗口请使用往常的 modal skills 列表的形式，skills set 的 delete 只会删除这个集合定义，不会删除后台的 skills，add 出现的 modal 窗口可以组合 set，后续的agent添加 set 的时候代表的是多选 item，即添加了 一个 set 就会将该 set 下的所有 skills 选择并返回到 modal 窗口下的 skills item的页面；
4. [x] 这里的 update 只会更新具有使用第一步添加的信息来更新对应 update 的功能，具体逻辑可以参考 C:\Users\Administrator\Downloads\skills-manager-main\skills-manager-main\ 项目；skills 为 set 的 update 是指批量更新该 set 下的 skills
5. [x] 整理逻辑，去掉没必要的逻辑和测试，将 src-tauri/src 路径下的内容尽可能的移动到 src-tauri/crates 的对应库中，并创建 yss-api 库，将前端需要的逻辑在这里处理，在 src-tauri/src 目录下只调用 yss-api 的逻辑包装一下发送给前端就好了
6. 完成 1~4 每一步的时候使用 git commit 提交并 push，第 5 步的时候每完成部分逻辑迁移就使用 git commit 提交并 push
