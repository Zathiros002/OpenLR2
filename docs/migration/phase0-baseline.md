# Phase 0 — 基线冻结

## 状态

📋 **待执行** — 需要在 Windows + OpenLR2 C++ 构建环境中运行。

## 目标

为 Rust 重构建立不可争辩的行为基准。

## 任务清单

- [ ] 编写 C++ dump 工具：
  - [ ] `-dump-bms <path>` — dump BMS parse 结果
  - [ ] `-dump-timing <path>` — dump timing timeline
  - [ ] `-dump-skin <path>` — dump skin SRC/DST/DrawingBuf
  - [ ] `-dump-replay <path>` — dump replay data

- [ ] 收集固定测试资源：
  - [ ] 7K 基础谱面 → `fixtures/bms/7k_simple/`
  - [ ] BPM/STOP 谱面 → `fixtures/bms/bpm_stop/`
  - [ ] LNOBJ 谱面 → `fixtures/bms/lnobj/`
  - [ ] Mine 谱面 → `fixtures/bms/mine/`
  - [ ] PMS 9K → `fixtures/bms/pms_9k/`
  - [ ] Base62 keysound → `fixtures/bms/base62/`
  - [ ] CP932 路径 → `fixtures/bms/cp932/`
  - [ ] 默认 LR2 skin → `fixtures/skins/default/`
  - [ ] HD skin → `fixtures/skins/hd/`

- [ ] 采集 Windows 基准：
  - [ ] 选歌截图
  - [ ] 游玩截图
  - [ ] Result 截图
  - [ ] autoplay score 记录

- [ ] 生成 golden JSON：
  - [ ] `fixtures/golden/bms/7k_simple.json`
  - [ ] `fixtures/golden/bms/bpm_stop.json`
  - [ ] `fixtures/golden/bms/lnobj.json`
  - [ ] `fixtures/golden/bms/mine.json`
  - [ ] `fixtures/golden/bms/pms_9k.json`

## 验收

- [ ] `fixtures/golden/*.json` 可重复生成
- [ ] C++ 当前行为被记录，不再靠记忆迁移
