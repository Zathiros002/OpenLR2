# 迁移进度记录

> 最后更新: 2026-06-01

## 总体状态

| Phase | 状态 | 开始日期 | 完成日期 |
|-------|------|----------|----------|
| Phase 0（基线冻结 — 基础设施） | ✅ 目录结构已建 | 2026-06-01 | 2026-06-01 |
| Phase 0（基线冻结 — golden 生成） | 📋 需 Windows 环境 | — | — |
| Phase 1（Rust workspace + BMS parser） | ✅ 已完成初版 | 2026-06-01 | 2026-06-01 |

---

## 2026-06-01 — 初始搭建

### 新建目录

```
OpenLR2/
├── rust/                          ← 新建 (Rust workspace)
│   ├── Cargo.toml                 ← workspace 根配置
│   ├── crates/
│   │   ├── openlr2-core/          ← 基础类型 crate
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── error.rs       ← 统一错误类型
│   │   │       ├── time.rs        ← 时间模型 (Millis/Beat/BmsTime/Micros)
│   │   │       ├── keymode.rs     ← 键位模式枚举 (5K/7K/9K/10K/14K)
│   │   │       ├── resource.rs    ← 资源句柄 (TextureId/SoundId/FontId)
│   │   │       └── encoding.rs    ← 编码 stub (CP932 ↔ UTF-8)
│   │   └── openlr2-bms/           ← BMS parser crate
│   │       ├── Cargo.toml
│   │       └── src/
│   │           ├── lib.rs
│   │           ├── chart.rs       ← Chart/Note/Lane/BmsMeta/ResourceTable
│   │           ├── parser.rs      ← BMS 行解析器 + 单元测试
│   │           └── timing.rs      ← BPM/STOP 时间线 + 单元测试
│   └── tools/
│       └── bms-dump/              ← CLI dump 工具
│           ├── Cargo.toml
│           └── src/main.rs
│
├── fixtures/                      ← 新建
│   ├── bms/                       (空，待填充谱面)
│   ├── skins/                     (空)
│   ├── replay/                    (空)
│   ├── golden/
│   │   └── bms/                   (空，待 Phase 0 生成)
│   └── README.md                  ← fixtures 说明
│
├── docs/
│   └── migration/
│       ├── golden-format.md       ← Golden 数据格式规范
│       └── phase0-baseline.md     ← Phase 0 任务清单
│
└── scripts/
    └── generate-golden/           (空，待放 dump 脚本)
```

### 新增/修改文件清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `rust/Cargo.toml` | 新建 | Rust workspace 根配置（3 个 member） |
| `rust/crates/openlr2-core/Cargo.toml` | 新建 | 依赖 thiserror, serde |
| `rust/crates/openlr2-core/src/lib.rs` | 新建 | 模块入口，re-export 所有公共类型 |
| `rust/crates/openlr2-core/src/error.rs` | 新建 | `OpenLr2Error` — 10 个错误变体 |
| `rust/crates/openlr2-core/src/time.rs` | 新建 | `Millis`, `Micros`, `Beat`, `BmsTime` |
| `rust/crates/openlr2-core/src/keymode.rs` | 新建 | `KeyMode` 枚举 + `TryFrom<u32>` + Serialize |
| `rust/crates/openlr2-core/src/resource.rs` | 新建 | `TextureId`, `SoundId`, `FontId` |
| `rust/crates/openlr2-core/src/encoding.rs` | 新建 | CP932 ↔ UTF-8 stub |
| `rust/crates/openlr2-bms/Cargo.toml` | 新建 | 依赖 openlr2-core, thiserror, serde, serde_json |
| `rust/crates/openlr2-bms/src/lib.rs` | 新建 | 模块入口 |
| `rust/crates/openlr2-bms/src/chart.rs` | 新建 | `Chart`, `BmsMeta`, `Lane`, `Note`, `NoteKind`, `ResourceTable`, `BmsParseOptions` |
| `rust/crates/openlr2-bms/src/parser.rs` | 新建 | BMS 行解析器 + 4 个测试 |
| `rust/crates/openlr2-bms/src/timing.rs` | 新建 | `BpmEvent`, `StopEvent`, `TimingEvent`, `build_timeline()` + 3 个测试 |
| `rust/tools/bms-dump/Cargo.toml` | 新建 | 依赖 openlr2-bms, serde_json |
| `rust/tools/bms-dump/src/main.rs` | 新建 | CLI: `bms-dump <path>` → JSON stdout |
| `fixtures/README.md` | 新建 | Fixtures 使用说明 |
| `docs/migration/golden-format.md` | 新建 | Golden JSON 格式规范 + 比较策略 |
| `docs/migration/phase0-baseline.md` | 新建 | Phase 0 任务清单 |

### 测试结果

```
running 7 tests
test parser::tests::base36_decode ................... ok
test parser::tests::channel_to_lane_7k .............. ok
test parser::tests::parse_minimal_bms ............... ok
test parser::tests::detect_keymode_from_player ...... ok
test timing::tests::single_bpm_no_stops ............. ok
test timing::tests::bpm_change_mid_song ............. ok
test timing::tests::stop_delays_time ................ ok

test result: ok. 7 passed; 0 failed
```

### BMS Parser 当前支持的功能

| 功能 | 状态 | 对应 C++ 源码 |
|------|------|--------------|
| `#TITLE` / `#SUBTITLE` / `#ARTIST` / etc. | ✅ 已实现 | `LR2_bmsload.cpp` |
| `#PLAYER` → KeyMode 检测 | ✅ 已实现 | `gameplay.keymode` |
| `#BPM` (初始 BPM) | ✅ 已实现 | — |
| `#WAVxx` / `#BMPxx` (Base36) | ✅ 已实现 | — |
| Channel 数据解析 (`#xxxnn:VV`) | ✅ 初版 | `LR2_bmsload.cpp` |
| BPM 通道 (ch 3/8) | ✅ 已实现 | — |
| STOP 通道 (ch 9) | ✅ 已实现 | — |
| Note 通道 (ch 11-17, 21-27) | ✅ 1P/2P | — |
| BPM timeline (`build_timeline`) | ✅ 已实现 | `gameplay.bpmt_data[]` |
| `bms_time_to_real()` 转换 | ✅ 已实现 | `RealTimeToBMSTime` 反向 |
| Base36 解码 | ✅ 已实现 | `Base36ToInt()` |
| `bms-dump` CLI 工具 | ✅ 可用 | — |

### BMS Parser 尚未实现的功能

| 功能 | 状态 | 优先级 |
|------|------|--------|
| `#RANDOM` / `#IF` 条件块 | ❌ 未实现 | 高 |
| Base62 资源索引 (`#WAV` 后缀为小写) | ❌ 未实现 | 高 |
| Long Note (LNOBJ) 配对 | ❌ 未实现 | 高 |
| Mine (地雷) 检测 | ❌ 未实现 | 中 |
| BGA 通道 (ch 4/6/7) | ❌ 未实现 | 中 |
| BGM 通道 (ch 1) | ❌ 未实现 | 低 |
| CP932 文件内容解码 | ❌ stub | 中 |
| DP/SP 轨道拆分 (`DPsplit`/`SPtoDP`) | ❌ 未实现 | 中 |
| Scratch 侧检测 | ❌ 未实现 | 中 |
| Chart MD5 哈希 | ❌ 未实现 | 中 |
| `#STOPxx` / `#STPxx` 独立行解析 | ⚠️ 时序位置未关联 | 低 |
| PMS 9K 扩展通道 | ❌ 未实现 | 低 |

---

## 下一步 (Phase 1 继续)

按 `RUST_SWIFT_REARCHITECTURE_PLAN.md` 第 21 节的建议：

1. 补全 BMS parser 高优先级功能（Base62, LNOBJ, RANDOM/IF）
2. 在 Windows 上运行 C++ dump 生成 golden 基准数据
3. 将 Rust `bms-dump` 输出与 golden JSON 进行 diff 比较
4. 逐个修复差异直到输出完全匹配

## 下一步 (Phase 2 预览)

- 实现 `openlr2-judge` crate：判定、血条、分数计算
- 实现 `openlr2-replay` crate：Replay 编解码
