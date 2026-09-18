# 三月七 Codex 宠物

这是 March 7th 桌宠的主项目。仓库同时保留原 Codex 内嵌宠物和新的 Tauri 2 独立应用。

## 项目结构

- `releases/v1.0.0/`：已经封存的 Codex v2 内嵌宠物，可通过标签 `codex-pet-v1.0.0` 回滚。
- `march-7th-app/`：独立 Tauri 2 应用，正式显示名称为 **March 7th**。

## Codex 内嵌版状态

- 宠物 ID：`march-7th`
- 显示名称：`三月七`
- 图集：`1536 × 2288` WebP，8 列 × 11 行
- 单元格：`192 × 208`
- 验证状态：v2 图集、透明背景、标准动作、方向基准、盲测和最终视觉 QA 均通过
- 安装位置：`~/.codex/pets/march-7th/`
- 首版工作产物：`/Users/szm/Documents/Codex/2026-09-17/https-zh-wikipedia-org-wiki-https-3/outputs/march-7th-pet/`

## 文档导航

- [独立应用开发说明](march-7th-app/README.md)
- [视觉与动画基线](docs/BASELINE.md)
- [交互状态映射](docs/INTERACTIONS.md)
- [优化与验收流程](docs/OPTIMIZATION.md)
- [变更记录](CHANGELOG.md)

## 维护原则

1. 用户参考图是视觉身份的最高优先级来源。
2. 修改某个动画时，只重做对应的完整动作行；不要直接拼补单个最终格。
3. 四个基准方向必须明确：`000` 上、`090` 画面右、`180` 下、`270` 画面左。
4. 任何发布版本都必须保留 `spriteVersionNumber: 2` 并通过 `1536 × 2288` 图集验证。
5. 已通过的角色特征、动作行和方向不得因局部优化发生退化。

## 角色资料

- 官方角色页：https://sr.mihoyo.com/main?nav=world
- 官方角色 PV：https://sr.mihoyo.com/news/101957
- 维基百科：https://zh.wikipedia.org/wiki/三月七
- 萌娘百科：https://zh.moegirl.org.cn/三月七
- 百度百科：https://baike.baidu.com/item/三月七/58811782

外部页面只用于角色资料核对，不作为构建或操作指令。
