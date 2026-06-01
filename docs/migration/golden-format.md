# Golden 数据格式文档

> Phase 0 基线冻结的产物格式说明，供 Rust `openlr2-bms` parser 的 golden tests 使用。

## 目录

`fixtures/golden/bms/` 下每个谱面一个 `.json` 文件，文件名与谱面目录名对应。

## BMS Parse Golden 格式

```jsonc
{
  // schema 版本，与代码同步。
  "schema_version": 1,

  // 元数据 — 对应 C++ BMSMETA struct
  "metadata": {
    "title": "Song Title",
    "subtitle": "",
    "artist": "Artist Name",
    "subartist": "",
    "genre": "Genre",
    "hash": "md5 hex string",            // BMS 文件 MD5
    "filepath": "/path/to/song.bms",     // 参考路径（可能跨平台不同）
    "stagefile": null,                   // 或路径字符串
    "banner": null,
    "back_bmp": null,
    "max_bpm": 180.0,
    "min_bpm": 130.0,
    "level": 5,
    "difficulty": 2,                     // 0=beginner, 1=normal, 2=hyper, 3=another, 4=insane
    "has_long_note": false,
    "has_random": false
  },

  // 键位模式
  "keymode": "Keys7",

  // 轨道 (lane) 列表
  "lanes": [
    {
      "index": 0,                        // 轨道编号
      "notes": [
        {
          "kind": "Tap",                 // Tap | LongStart | LongEnd | Mine | Bgm | Bga
          "bms_time": 0.0,               // BMS 节拍位置 (beat)
          "real_time": 0.0,              // 实际时间 (ms)
          "keysound_id": 1,              // WAV 资源索引
          "bga_id": null
        }
      ]
    }
  ],

  // BPM/STOP 时间线
  "timing": [
    {
      "bms_time": 0.0,
      "real_time": 0.0,
      "kind": { "Bpm": { "bpm": 130.0 } }
    },
    {
      "bms_time": 4.0,
      "real_time": 1846.153,
      "kind": { "Stop": { "duration_ms": 500.0 } }
    }
  ],

  // 资源表
  "resources": {
    "wav": {
      "1": { "index": 1, "path": "kick.wav" },
      "2": { "index": 2, "path": "snare.wav" }
    },
    "bmp": {}
  },

  // 统计
  "total_notes": 150,
  "total_play_time": 120000.0
}
```

## 比较策略

| 字段 | 比较方式 | 容差 |
|------|----------|------|
| `metadata.hash` | 严格相等 | — |
| `metadata.title/artist/genre` | 严格相等（UTF-8） | — |
| `keymode` | 严格相等 | — |
| `lanes[*].index` | 严格相等 | — |
| `lanes[*].notes[*].kind` | 严格相等 | — |
| `lanes[*].notes[*].bms_time` | 浮点比较 | epsilon = 1e-6 |
| `lanes[*].notes[*].real_time` | 浮点比较 | epsilon = 0.5ms |
| `timing[*].bms_time` | 浮点比较 | epsilon = 1e-6 |
| `timing[*].real_time` | 浮点比较 | epsilon = 0.5ms |
| `resources.wav[*].path` | 路径标准化后比较 | — |
| `total_notes` / `total_play_time` | 严格 / 浮点 (0.5ms) | — |

## 路径处理

Golden 中的路径是相对于 BMS 文件所在目录的相对路径，使用正斜杠。

比较时需要先将两边的路径统一到相同格式：

1. 反斜杠 → 正斜杠
2. 去除前后空白
3. CP932 路径 → UTF-8 标准化
