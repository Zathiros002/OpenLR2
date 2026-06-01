# Fixtures

测试资源目录。Phase 0 基线冻结和 Phase 1+ Rust parser 的 golden tests 使用。

## 目录结构

```
fixtures/
├── bms/           # BMS 测试谱面
│   ├── 7k_simple/        # 简单 7K 谱面
│   ├── bpm_stop/         # BPM 变化 + STOP 谱面
│   ├── lnobj/            # 长条 (LN) 谱面
│   ├── mine/             # 地雷谱面
│   ├── pms_9k/           # PMS 9K 谱面
│   ├── base62/           # Base62 按键音谱面
│   └── cp932/            # CP932 编码/路径谱面
├── skins/         # 测试皮肤
│   ├── default/          # LR2 默认皮肤
│   └── hd/               # HD 皮肤
├── replay/        # 测试 Replay 文件
└── golden/        # Golden 基准数据 (Phase 0 产物)
    └── bms/              # 每个 BMS 对应一个 .json
```

## 使用

### Phase 0: 生成 golden 基准

在 Windows + OpenLR2 C++ 构建中运行 dump 工具：

```
openlr2.exe -dump-bms fixtures/bms/7k_simple/song.bms > fixtures/golden/bms/7k_simple.json
```

### Phase 1: Rust parser 验证

```
cd rust
cargo run -p bms-dump -- ../fixtures/bms/7k_simple/song.bms
diff <(cargo run ...) ../fixtures/golden/bms/7k_simple.json
```
