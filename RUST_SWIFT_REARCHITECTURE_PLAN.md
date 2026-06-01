# OpenLR2 Rust 后端 + Swift macOS/iOS 壳重构移植全流程方案

> 目标：保留 OpenLR2/LR2 的核心结构、数据兼容和玩法语义，用 Rust 重构跨平台后端/runtime，用 Swift/SwiftUI/AppKit/UIKit 负责 macOS/iOS 平台壳、窗口生命周期、系统集成和 Apple 平台体验。
>
> 核心原则：**不是“换语言重写一个长得像 LR2 的播放器”，而是把 LR2 的 BMS、皮肤、判定、Replay、Score、目录兼容等行为固化成可测试的 Rust runtime，再让 Swift 壳调用它。**

---

## 1. 战略判断

### 1.1 为什么不是直接移植现有 C++/DxLib

现有代码能跑的关键基础是 Windows 生态：

- DxLib 承担窗口、图形、输入、图片、字体、部分文件/日志语义。
- FMOD 当前仓库内只有 Windows x86/x64 预编译库。
- MIDI 使用 WinMM。
- CustomIR 使用 Windows DLL ABI。
- `Main.cpp`、`Scene*.cpp`、`LR2_skindraw.cpp`、`LR2_skinobject.cpp` 等处直接或间接依赖 DxLib/Win32。
- `structure.h` 的 `game` 大结构把配置、皮肤、输入、音频、数据库、场景、网络和游玩状态强耦合在一起。

硬移植的结果通常是：为了让 C++ 继续编译，不断堆条件编译和 stub，最后得到一个维护困难的跨平台分叉。

### 1.2 为什么 Rust 做后端

Rust 适合承接 OpenLR2 最有长期价值的部分：

- BMS/PMS 解析。
- BPM/time line 和 note 调度。
- 判定、血条、分数、Replay、Ghost。
- LR2 皮肤脚本解析、SRC/DST 时间线求值。
- SQLite/配置/路径/编码兼容。
- 跨平台音频、输入、资源加载、网络任务调度。
- 可测试、可 fuzz、可持续迁移的核心逻辑。

### 1.3 为什么 Swift 做 macOS/iOS 壳

Swift 适合 Apple 平台壳：

- AppKit/SwiftUI/UIKit 生命周期。
- macOS `.app` bundle、菜单栏、Preferences、文件选择器、沙盒目录。
- iOS UIKit/SwiftUI、触控、外接键盘、Game Controller。
- Metal layer、CAMetalLayer、Retina/high-DPI。
- CoreMIDI、CoreAudio/AudioSession、GameController。
- codesign、notarization、App Store/side-load 打包。

Swift 不建议承担全平台核心逻辑，因为 Linux/Windows/Android 生态和 GUI/音频/渲染后端会立刻变窄。Swift 应该是 Apple shell，不是唯一 runtime。

### 1.4 目标形态

```text
OpenLR2 Next
├── openlr2-core        Rust：BMS、timing、judge、score、replay、db model
├── openlr2-skin        Rust：LR2 skin parser、SRC/DST evaluator、draw commands
├── openlr2-runtime     Rust：scene runtime、resource manager、scheduler
├── openlr2-audio       Rust：audio backend traits + FMOD/miniaudio/CoreAudio adapter
├── openlr2-render      Rust：renderer traits + wgpu/Metal/SDL bridge
├── openlr2-platform    Rust：filesystem、config dirs、threads、logging、network
├── openlr2-ffi         Rust：C ABI + Swift-friendly API
├── apps/macos          Swift：AppKit/SwiftUI shell
├── apps/ios            Swift：UIKit/SwiftUI shell
├── apps/desktop        可选 Rust/SDL shell：Linux/Windows 调试壳
└── legacy-cpp          当前 C++ 代码，作为行为参考和渐进迁移来源
```

---

## 2. 保留什么，重构什么

### 2.1 必须保留的兼容语义

| 领域 | 兼容目标 |
|------|----------|
| BMS/PMS | 支持现有 BMS 解析规则、Base62、BPM/STOP/LN/MINE、keymode |
| Timing | `RealTimeToBMSTime`、BPM timeline、主 BPM high-speed 语义 |
| 判定 | PGREAT/GREAT/GOOD/BAD/POOR/MINE、LN 判定、autoplay、replay 输入 |
| Score | EXScore、combo、clear lamp、playcount、clearcount、failcount |
| Gauge | Normal/Hard/ExHard/Easy/Assist/GAS 等语义 |
| Replay/Ghost | 旧数据可读，新数据尽量可被旧工具理解 |
| SQLite | `LR2files/Database/Score/*.db` 和 `song.db` 兼容 |
| Skin | LR2 skin CSV/opcode、SRC/DST、option、customize、DrawingBuf 语义 |
| 资源目录 | 原 `LR2files/` 布局兼容模式 |
| 编码 | UTF-8、CP932、Windows 旧资源路径兼容 |

### 2.2 可以重构的实现细节

| 现有实现 | 新实现 |
|----------|--------|
| DxLib 直接绘制 | Rust 生成 `DrawCommand[]`，渲染后端执行 |
| `game` 巨型结构 | Rust domain model + runtime state + scene state 分离 |
| CSTR | Rust `String`/`PathBuf` + encoding adapter |
| `int` 图形句柄 | `ResourceId<Texture>`、`ResourceId<Font>` |
| FMOD 指针散落在结构体 | `AudioBackend` trait + opaque handle |
| WinMM MIDI | `MidiBackend` trait，macOS CoreMIDI，Linux RtMidi/ALSA |
| Windows DLL CustomIR | 禁用第一阶段；后续外部进程协议或 C ABI plugin |
| 线程直接访问状态 | command queue + resource loader + main-thread renderer |

---

## 3. 新 Rust 后端架构

### 3.1 Workspace 设计

建议新建 Rust workspace：

```text
rust/
├── Cargo.toml
├── crates/
│   ├── openlr2-core/
│   ├── openlr2-bms/
│   ├── openlr2-skin/
│   ├── openlr2-runtime/
│   ├── openlr2-render/
│   ├── openlr2-audio/
│   ├── openlr2-platform/
│   ├── openlr2-db/
│   ├── openlr2-ir/
│   ├── openlr2-ffi/
│   └── openlr2-testkit/
└── tools/
    ├── bms-dump/
    ├── skin-dump/
    ├── replay-verify/
    └── golden-runner/
```

### 3.2 crate 职责

| Crate | 职责 | 不应该做 |
|-------|------|----------|
| `openlr2-core` | 通用类型、时间、判定、score、gauge、game model | 不碰文件 IO、渲染、音频 |
| `openlr2-bms` | BMS/PMS parser、chart model、timing timeline | 不加载音频/图片资源 |
| `openlr2-skin` | LR2 skin parser、SRC/DST、animation evaluator、draw command 生成 | 不调用 GPU API |
| `openlr2-runtime` | Scene runtime、状态机、主循环 tick、资源调度 | 不绑定 AppKit/UIKit |
| `openlr2-render` | renderer trait、wgpu/Metal/SDL backend adapter | 不懂 BMS 判定 |
| `openlr2-audio` | audio trait、sound cache、channel groups、latency config | 不懂 skin |
| `openlr2-platform` | 路径、日志、配置目录、线程、动态库、编码 | 不保存游戏分数逻辑 |
| `openlr2-db` | SQLite 读写、score/song db 兼容 | 不做 UI 查询逻辑 |
| `openlr2-ir` | IR HTTP、CustomIR 新协议 | 不直接阻塞 runtime tick |
| `openlr2-ffi` | C ABI、Swift bridge、opaque handle 管理 | 不包含业务算法 |
| `openlr2-testkit` | golden tests、fixtures、截图/音频基准工具 | 不进入发布包 |

### 3.3 数据流

```text
Swift App Shell
    ↓ lifecycle/input/window/audio session
openlr2-ffi
    ↓ stable C ABI
openlr2-runtime
    ↓ tick/update/render list
openlr2-core + openlr2-bms + openlr2-skin
    ↓ commands/resources
openlr2-render / openlr2-audio / openlr2-platform
    ↓
Metal/wgpu/CoreAudio/FMOD/CoreMIDI/filesystem/network
```

### 3.4 主循环模型

Rust runtime 不应该自己拥有 Apple app main loop。Swift 壳负责驱动：

```text
CADisplayLink / CVDisplayLink / MTKView draw callback
    ↓
openlr2_runtime_tick(runtime, delta_time_ns)
    ↓
openlr2_runtime_collect_audio_commands()
openlr2_runtime_collect_render_commands()
    ↓
Swift/Metal 或 Rust render backend 执行
```

macOS 可选：

- AppKit + `MTKView`。
- SwiftUI 包 `NSViewRepresentable` 包装 `MTKView`。
- 低延迟场景优先 AppKit/MTKView，SwiftUI 只做设置窗口和外围 UI。

iOS 可选：

- UIKit + `MTKView`。
- SwiftUI 包 `UIViewRepresentable`。
- 外接键盘/GameController/触控转成统一 input events。

---

## 4. Rust Domain Model 设计

### 4.1 从 C++ `structure.h` 拆分

现有 `structure.h` 中关键结构：

- `game`
- `gameplay`
- `skstruct`
- `SRCstruct`
- `DSTstruct`
- `DSTdraw`
- `DrawingBuf`
- `SONGDATA`
- `STATUS`
- `PLAYERSTATUS`
- `SOUNDDATA`

Rust 中不要照搬一个巨大结构，而是拆成：

```rust
pub struct AppState {
    pub config: Config,
    pub profile: PlayerProfile,
    pub scene: SceneState,
    pub runtime_flags: RuntimeFlags,
}

pub enum SceneState {
    SongSelect(SongSelectState),
    Decide(DecideState),
    Play(PlayState),
    Result(ResultState),
    KeyConfig(KeyConfigState),
    SkinSelect(SkinSelectState),
    CourseResult(CourseResultState),
}

pub struct PlayState {
    pub chart: LoadedChart,
    pub timing: TimingTimeline,
    pub players: [PlayerPlayState; 2],
    pub replay: ReplayState,
    pub ghost: GhostState,
    pub audio_plan: AudioPlan,
    pub render_plan: PlayRenderState,
}
```

### 4.2 句柄和资源

不要使用裸 `int` 复刻所有句柄。建议：

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ResourceId<T> {
    raw: u32,
    _marker: std::marker::PhantomData<T>,
}

pub enum Texture {}
pub enum Sound {}
pub enum Font {}
pub enum Movie {}

pub type TextureId = ResourceId<Texture>;
pub type SoundId = ResourceId<Sound>;
pub type FontId = ResourceId<Font>;
```

FFI 层可以暴露 `u32`，Rust 内部保持类型安全。

### 4.3 时间模型

需要明确三种时间：

| 时间 | 含义 |
|------|------|
| `WallTime` | 系统真实时间，驱动 tick |
| `SceneTime` | 当前场景开始后的毫秒/微秒 |
| `ChartTime` | BMS timeline 上的音乐时间 |

建议用强类型：

```rust
#[derive(Clone, Copy, Debug)]
pub struct Millis(pub i64);

#[derive(Clone, Copy, Debug)]
pub struct Micros(pub i64);

#[derive(Clone, Copy, Debug)]
pub struct Beat(pub f64);

#[derive(Clone, Copy, Debug)]
pub struct BmsTime(pub f64);
```

### 4.4 错误模型

核心 crate 使用 `thiserror`：

```rust
#[derive(thiserror::Error, Debug)]
pub enum OpenLr2Error {
    #[error("failed to parse BMS: {0}")]
    BmsParse(String),
    #[error("resource not found: {path}")]
    ResourceNotFound { path: PathBuf },
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}
```

FFI 层不抛 Rust panic，不跨 FFI 传 Rust error。统一返回 `OpenLr2Status`，错误详情可由 `openlr2_last_error()` 取出。

---

## 5. BMS/PMS 迁移方案

### 5.1 迁移来源

现有重点文件：

- `LR2/LR2_bmsload.cpp`
- `LR2/LR2_bmsload.h`
- `LR2/Scene04_Play.cpp` 中与 `ParseBmsFile` 后续使用相关的逻辑
- `LR2/En_fileutil.cpp` 中 hash、编码、文件判断

### 5.2 Rust 模型

```rust
pub struct Chart {
    pub metadata: ChartMetadata,
    pub keymode: KeyMode,
    pub lanes: Vec<Lane>,
    pub timing: TimingTimeline,
    pub resources: ChartResources,
    pub total_notes: u32,
    pub total_play_time: Millis,
}

pub struct Lane {
    pub index: u8,
    pub notes: Vec<Note>,
}

pub enum NoteKind {
    Tap,
    LongStart { end_time: BmsTime },
    LongEnd,
    Mine { damage: i32 },
    Bgm,
    Bga,
}

pub struct TimingEvent {
    pub bms_time: BmsTime,
    pub real_time: Millis,
    pub kind: TimingEventKind,
}
```

### 5.3 Parser 兼容要求

必须覆盖：

- `#PLAYER`
- `#GENRE`
- `#TITLE`
- `#SUBTITLE`
- `#ARTIST`
- `#BPM`
- `#BPMxx`
- `#STOPxx`
- `#WAVxx`
- `#BMPxx`
- `#RANDOM/#IF/#ENDIF`
- Base36/Base62 resource ids。
- 5K/7K/9K/10K/14K。
- LNOBJ 和长音符规则。
- BGA/layer/poor BGA。
- 文件编码：UTF-8、CP932、带 BOM/无 BOM。

### 5.4 测试策略

为 parser 建 golden tests：

```text
fixtures/bms/
├── simple_7k/
├── bpm_stop/
├── longnote_lnobj/
├── base62_keysounds/
├── pms_9k/
├── cp932_path/
└── random_if/
```

每个 fixture 输出：

```json
{
  "hash": "...",
  "title": "...",
  "keymode": 7,
  "total_notes": 1234,
  "lanes": [...],
  "bpm_events": [...],
  "stop_events": [...],
  "resource_count": {
    "wav": 120,
    "bmp": 14
  }
}
```

验收：

- Rust parser 输出与 C++ dump 工具一致。
- 随机分支可指定 seed，结果稳定。
- 路径大小写和编码问题有 regression cases。

---

## 6. 判定、Gauge、Score、Replay 迁移方案

### 6.1 迁移来源

重点文件：

- `LR2/Scene04_Play.cpp`
  - `ProcGame`
  - `ProcNoteOnTiming`
  - `ApplyJudgeNote`
  - `ApplyJudgeMine`
  - `JudgeToScore`
  - `DrawNotes` 中部分 note active 状态逻辑
- `LR2/LR2_statplay.cpp`
- `LR2/LR2_replay.cpp`
- `LR2/LR2_ghost.cpp`
- `LR2/structure.h` 的 `PLAYERSTATUS`

### 6.2 纯逻辑 API

判定系统必须从渲染和音频中独立出来：

```rust
pub struct JudgeEngine {
    chart: Arc<LoadedChart>,
    rules: JudgeRules,
    state: JudgeState,
}

impl JudgeEngine {
    pub fn update(&mut self, chart_time: BmsTime, input: &InputFrame) -> Vec<JudgeEvent>;
    pub fn apply_replay_frame(&mut self, frame: ReplayFrame) -> Vec<JudgeEvent>;
    pub fn finish(self) -> PlayResult;
}
```

### 6.3 JudgeEvent

```rust
pub struct JudgeEvent {
    pub player: PlayerSide,
    pub lane: LaneIndex,
    pub note_id: NoteId,
    pub kind: JudgeKind,
    pub offset_ms: i32,
    pub chart_time: BmsTime,
}

pub enum JudgeKind {
    PGreat,
    Great,
    Good,
    Bad,
    Poor,
    Mine,
}
```

### 6.4 Gauge/Score

```rust
pub struct ScoreState {
    pub pgreat: u32,
    pub great: u32,
    pub good: u32,
    pub bad: u32,
    pub poor: u32,
    pub combo: u32,
    pub max_combo: u32,
    pub ex_score: u32,
    pub clear: ClearLamp,
}

pub struct GaugeState {
    pub gauge_type: GaugeType,
    pub value: f32,
    pub survival: bool,
}
```

### 6.5 Replay

Replay 要支持两层：

1. **Legacy replay compatibility**：读取旧 OpenLR2/LR2 数据。
2. **New replay model**：Rust 原生、版本化、可扩展。

```rust
pub struct Replay {
    pub version: ReplayVersion,
    pub chart_hash: String,
    pub options: PlayOptions,
    pub seed: RandomSeed,
    pub frames: Vec<ReplayFrame>,
}

pub enum ReplayFrame {
    Input { time: Millis, player: PlayerSide, button: Button, pressed: bool },
    Judge { time: Millis, player: PlayerSide, lane: LaneIndex, judge: JudgeKind },
}
```

### 6.6 验收

- 同一个 BMS + 同一个 replay，Rust 输出的判定数、EXScore、clear lamp、max combo 与 C++ 基准一致。
- Autoplay 结果稳定。
- Random seed 固定时 note 排布一致。
- Long note、mine、battle、DP、PMS 有专门 fixture。

---

## 7. Skin Runtime 迁移方案

### 7.1 核心思想

不要让 skin runtime 直接画图。Rust skin runtime 做三件事：

1. 解析 LR2 skin CSV/opcode/customize。
2. 根据场景状态和时间求值 SRC/DST。
3. 输出平台无关的 `DrawCommand[]`。

```text
SkinScript + SceneState + Time
        ↓
SkinEvaluator
        ↓
Vec<DrawCommand>
        ↓
RendererBackend
```

### 7.2 迁移来源

重点文件：

- `LR2/LR2_skinload.cpp`
- `LR2/LR2_skindraw.cpp`
- `LR2/LR2_skinobject.cpp`
- `LR2/LR2_skinmanage.cpp`
- `OpcodeScript/`
- `LR2/structure.h` 的 `skstruct`、`SRCstruct`、`DSTstruct`、`DSTdraw`、`DrawingBuf`

### 7.3 Rust 模型

```rust
pub struct Skin {
    pub metadata: SkinMetadata,
    pub resources: SkinResources,
    pub objects: Vec<SkinObject>,
    pub options: SkinOptions,
    pub custom: SkinCustomize,
}

pub struct Src {
    pub resource: ResourceRef,
    pub rect: Rect,
    pub frames: FrameSpec,
    pub cycle: Option<Cycle>,
}

pub struct Dst {
    pub option_condition: OptionCondition,
    pub timeline: Vec<DstKeyframe>,
}

pub struct DstKeyframe {
    pub time: Millis,
    pub rect: Rect,
    pub alpha: u8,
    pub angle: f32,
    pub blend: BlendMode,
    pub color: Color,
}
```

### 7.4 DrawCommand

```rust
pub enum DrawCommand {
    Image {
        texture: TextureId,
        src: Rect,
        dst: Rect,
        color: Color,
        alpha: f32,
        blend: BlendMode,
        rotation: Rotation,
        z: i32,
    },
    Text {
        font: FontId,
        text: String,
        dst: Point,
        color: Color,
        alpha: f32,
        align: TextAlign,
        z: i32,
    },
    Bar {
        texture: TextureId,
        src: Rect,
        dst: Rect,
        fill: f32,
        direction: BarDirection,
        z: i32,
    },
    ClipBegin(Rect),
    ClipEnd,
}
```

### 7.5 Skin object 交互

按钮、滑块、鼠标 hover 等不要在 Swift UI 层实现，而应进入 Rust runtime：

```rust
pub enum SkinHitEvent {
    Button { id: SkinObjectId, action: SkinAction },
    Slider { id: SkinObjectId, value: i32 },
    Hover { id: SkinObjectId },
}
```

Swift 只传入 pointer/touch/mouse events。

### 7.6 OpcodeScript 处理

当前 `OpcodeScript/` 生成 C 头文件。新方案：

- 保留 opcode 定义文件。
- 增加 Rust 生成器，生成：
  - `skin_opcodes.rs`
  - opcode enum。
  - parser table。
  - 文档 dump。

也可以第一阶段手写 parser table，等稳定后再自动生成。

### 7.7 验收

- 解析默认 LR2 皮肤。
- 解析 HD skin。
- 输出 DrawCommand 数量和关键字段与 C++ DrawingBuf dump 接近。
- 选歌、游玩、Result 三类皮肤先通过。
- 支持 `#RESOLUTION`。
- 支持 skin customize 读取/保存。

---

## 8. 渲染后端方案

### 8.1 总体选择

Rust 后端可以有两种渲染执行方式：

| 方式 | 描述 | 适用 |
|------|------|------|
| Rust 执行渲染 | Swift 提供 native window/Metal layer，Rust/wgpu 渲染 | macOS/iOS 长期推荐 |
| Swift 执行渲染 | Rust 只返回 DrawCommand，Swift/Metal 执行 | Apple 平台深度集成推荐 |

建议最终走 **Rust 生成 DrawCommand + Swift/Metal 执行渲染** 或 **Rust/wgpu 执行渲染** 二选一，不要长期双维护。

### 8.2 Swift/Metal 渲染路线

优点：

- Apple 平台性能和调试体验好。
- Retina、MTKView、资源生命周期自然。
- iOS 更顺。

缺点：

- Linux/Windows 需要另一个 renderer。
- Rust DrawCommand 到 Swift 的 FFI 数据结构要设计稳定。

### 8.3 Rust/wgpu 渲染路线

优点：

- macOS/iOS/Linux/Windows 可共享大量渲染代码。
- Rust 资源管理统一。

缺点：

- iOS 集成、binary size、Metal layer 绑定复杂一些。
- Swift 壳需要把 CAMetalLayer 交给 Rust。

### 8.4 推荐

如果目标包括 Linux/macOS/iOS 长线：**Rust/wgpu 为主，Swift 壳只提供 surface/layer 和 app lifecycle**。

如果 Apple 平台优先且 Linux 稍后：**Swift/Metal 先跑通，然后再给 Linux 做 wgpu/SDL renderer**。

本方案推荐：

1. Phase A：Rust 生成 DrawCommand，先用 debug renderer dump。
2. Phase B：macOS Swift/Metal renderer 执行 DrawCommand，快速验证 Apple 壳。
3. Phase C：抽象 renderer backend，补 Rust/wgpu 或 SDL renderer，服务 Linux。

### 8.5 Metal renderer 最小功能

- Texture atlas 或独立 texture。
- Alpha blend。
- Additive blend。
- Color modulation。
- Rotation/scale。
- Scissor/clip。
- Render target。
- Text glyph atlas。
- High-DPI coordinate mapping。

### 8.6 像素对齐

LR2 皮肤极依赖 640x480 逻辑坐标。必须固定：

```text
logical_size = skin resolution 或 640x480
drawable_size = window_size * backing_scale
scale = min(drawable_width / logical_width, drawable_height / logical_height)
letterbox = center
```

所有 DrawCommand 使用逻辑坐标，由 renderer 做 scale/letterbox。

---

## 9. 音频后端方案

### 9.1 音频需求

BMS 音频需求比普通播放器高：

- 大量短 keysound 并发。
- 低延迟触发。
- BGM 与 keysound 分组音量。
- preview 淡入淡出。
- pitch/speed 调整。
- 暂停/恢复。
- iOS AudioSession 中断处理。

### 9.2 Rust audio trait

```rust
pub trait AudioBackend {
    fn init(&mut self, config: AudioConfig) -> Result<()>;
    fn load_sound(&mut self, path: &Path, flags: SoundFlags) -> Result<SoundId>;
    fn play(&mut self, sound: SoundId, params: PlayParams) -> Result<ChannelId>;
    fn stop(&mut self, channel: ChannelId);
    fn set_channel_volume(&mut self, channel: ChannelId, volume: f32);
    fn set_group_volume(&mut self, group: AudioGroup, volume: f32);
    fn set_pitch(&mut self, channel: ChannelId, pitch: f32);
    fn update(&mut self);
}
```

### 9.3 后端选择

| 后端 | 优点 | 缺点 | 建议 |
|------|------|------|------|
| FMOD | 与现有行为接近，功能完整 | 许可/SDK/打包复杂 | 第一阶段可用 |
| miniaudio | 跨平台、单头、Rust 可绑定 | 高级混音/解码/调参要自己做 | 长期可评估 |
| CoreAudio/AVAudioEngine | Apple 原生 | Linux/Windows 不复用 | Swift 壳专用，不建议做核心唯一后端 |
| rodio/cpal | Rust 生态 | 低延迟 keysound 大量并发需验证 | 原型可用 |

推荐：

- 第一阶段：FMOD Rust binding 或 C wrapper。
- 第二阶段：抽象完善后评估 miniaudio。
- iOS：如果 FMOD 可用且许可允许，优先 FMOD；否则 CoreAudio/miniaudio。

### 9.4 音频资源加载策略

- BGM/长音频 streaming。
- keysound 小文件预解码或内存缓存。
- preview 独立 channel group。
- stage keysound 按 chart resource table 分配。

后台加载线程不能直接触碰 UI/renderer，但可以解码音频并通过 command queue 注册。

### 9.5 验收

- 低延迟按键音，主观无明显滞后。
- 高密度谱面不爆音、不漏音。
- 自动播放与音频对齐。
- iOS 接电话/切后台后恢复正常。

---

## 10. 输入、MIDI、控制器方案

### 10.1 统一 InputEvent

Swift/AppKit/UIKit/GameController/CoreMIDI 事件全部转成 Rust input event：

```rust
pub enum InputEvent {
    Key { code: KeyCode, pressed: bool, repeat: bool },
    PointerMove { id: PointerId, x: f32, y: f32 },
    PointerButton { id: PointerId, button: PointerButton, pressed: bool },
    Touch { id: TouchId, phase: TouchPhase, x: f32, y: f32 },
    GamepadButton { id: GamepadId, button: GamepadButton, pressed: bool },
    MidiNote { device: MidiDeviceId, note: u8, velocity: u8, pressed: bool },
    MidiControl { device: MidiDeviceId, control: u8, value: u8 },
}
```

### 10.2 macOS 输入

Swift 壳负责：

- `NSEvent` keyboard。
- `flagsChanged` 修饰键。
- 鼠标/触控板。
- GameController framework。
- CoreMIDI input。

Rust 负责：

- key config mapping。
- Start/Select 组合。
- 5K/7K/9K/14K button mapping。
- skin object hit-test。

### 10.3 iOS 输入

iOS 需要额外设计：

- 外接键盘：UIKit key commands / presses。
- GameController。
- 触控虚拟按键，可选。
- MIDI 外设，CoreMIDI。
- 屏幕方向、safe area、home indicator。

注意：BMS 游戏在 iOS 上如果没有外设，触控体验会和桌面完全不同。建议 iOS 第一阶段目标是“播放器/观赏/autoplay/外接键盘可玩”，不要一开始承诺纯触控完整体验。

### 10.4 验收

- macOS 外接键盘 7K 可玩。
- macOS MIDI note on/off 可映射。
- iOS 外接键盘可触发按键。
- GameController 可映射 Start/Select/方向。
- 触控 UI 不影响 LR2 皮肤逻辑坐标。

---

## 11. Swift 壳设计

### 11.1 macOS App 结构

```text
apps/macos/OpenLR2Mac/
├── OpenLR2MacApp.swift
├── AppDelegate.swift
├── GameViewController.swift
├── GameMetalView.swift
├── PreferencesWindow.swift
├── LibraryPicker.swift
├── RustBridge/
│   ├── OpenLR2Bridge.swift
│   ├── OpenLR2Types.swift
│   └── OpenLR2FFI.modulemap
└── Resources/
```

职责：

- 创建窗口和 `MTKView`。
- 初始化 Rust runtime。
- 将 app lifecycle 转发给 Rust。
- 转发 keyboard/mouse/gamepad/midi events。
- 管理菜单栏、Preferences、文件选择器。
- 处理 `.app` bundle resource path 和 user data path。
- 显示错误弹窗和日志窗口。

### 11.2 iOS App 结构

```text
apps/ios/OpenLR2iOS/
├── OpenLR2iOSApp.swift
├── GameViewController.swift
├── GameMetalView.swift
├── DocumentPicker.swift
├── ExternalKeyboardHandler.swift
├── GameControllerManager.swift
├── MidiManager.swift
└── RustBridge/
```

职责：

- `MTKView` 渲染。
- iOS lifecycle：foreground/background/interruption。
- Documents/import BMS/LR2files。
- Files app integration。
- iCloud 可选。
- 外接键盘/GameController/CoreMIDI。
- AudioSession。

### 11.3 Swift 与 Rust 的职责边界

Swift 做：

- 平台生命周期。
- Native 文件选择器。
- 菜单/设置窗口。
- Metal surface。
- Apple API input/audio session/midi permission。
- Crash/report/log viewer。

Rust 做：

- 游戏状态。
- 场景状态。
- BMS/skin/score/replay。
- 资源索引。
- draw/audio commands。
- 配置读写。
- IR/network 业务。

Swift 不直接修改游戏内部状态，只发送 command：

```swift
bridge.send(.key(code: .z, pressed: true))
bridge.send(.openLibrary(url))
bridge.send(.setOption(.gauge, value: 2))
```

Rust 返回 snapshot/event：

```swift
let frame = bridge.tick(deltaTime)
renderer.draw(frame.drawCommands)
audio.apply(frame.audioCommands)
```

---

## 12. FFI 设计

### 12.1 ABI 原则

Rust 与 Swift 之间使用 C ABI：

- 不跨 FFI 传 Rust `String`、`Vec`、trait object。
- 不跨 FFI 传 Swift object。
- 所有对象用 opaque pointer/handle。
- 所有数组用 pointer + length。
- Rust 分配的内存由 Rust 释放。
- Swift 侧只持有生命周期明确的 handle。
- panic 不允许穿过 FFI。

### 12.2 Rust FFI 类型

```rust
#[repr(C)]
pub struct OpenLr2RuntimeHandle {
    _private: [u8; 0],
}

#[repr(C)]
pub enum OpenLr2Status {
    Ok = 0,
    Error = 1,
    InvalidArgument = 2,
    NotReady = 3,
}

#[repr(C)]
pub struct OpenLr2StringView {
    pub ptr: *const u8,
    pub len: usize,
}

#[repr(C)]
pub struct OpenLr2Buffer {
    pub ptr: *const u8,
    pub len: usize,
}
```

### 12.3 核心 FFI API

```c
OpenLr2Status openlr2_runtime_create(
    const OpenLr2RuntimeConfig* config,
    OpenLr2RuntimeHandle** out_runtime
);

void openlr2_runtime_destroy(OpenLr2RuntimeHandle* runtime);

OpenLr2Status openlr2_runtime_tick(
    OpenLr2RuntimeHandle* runtime,
    uint64_t delta_ns,
    OpenLr2FrameOutput* out_frame
);

OpenLr2Status openlr2_runtime_send_input(
    OpenLr2RuntimeHandle* runtime,
    const OpenLr2InputEvent* event
);

OpenLr2Status openlr2_runtime_command(
    OpenLr2RuntimeHandle* runtime,
    const OpenLr2Command* command
);

OpenLr2StringView openlr2_last_error(void);
```

### 12.4 DrawCommand FFI

为避免每帧大量跨语言对象分配，建议用 POD 数组：

```c
typedef enum OpenLr2DrawCommandKind {
    OPENLR2_DRAW_IMAGE,
    OPENLR2_DRAW_TEXT,
    OPENLR2_DRAW_CLIP_BEGIN,
    OPENLR2_DRAW_CLIP_END
} OpenLr2DrawCommandKind;

typedef struct OpenLr2DrawCommand {
    OpenLr2DrawCommandKind kind;
    uint32_t resource_id;
    float src_x, src_y, src_w, src_h;
    float dst_x, dst_y, dst_w, dst_h;
    float rgba[4];
    float rotation;
    int32_t blend_mode;
    int32_t z;
    uint32_t text_id;
} OpenLr2DrawCommand;
```

文字内容不直接塞 command。使用每帧 text table：

```c
typedef struct OpenLr2TextEntry {
    uint32_t id;
    const uint8_t* utf8;
    uintptr_t len;
} OpenLr2TextEntry;
```

### 12.5 Swift Bridge

Swift 包装层：

```swift
final class OpenLR2Runtime {
    private var handle: UnsafeMutablePointer<OpenLr2RuntimeHandle>?

    init(config: RuntimeConfig) throws
    func tick(deltaTime: Duration) throws -> FrameOutput
    func send(_ event: InputEvent) throws
    func command(_ command: RuntimeCommand) throws
    deinit
}
```

Swift 不直接暴露 C 指针给 UI 层。

### 12.6 生成绑定

推荐：

- Rust 使用 `cbindgen` 生成 C header。
- SwiftPM 使用 modulemap 引入 header。
- Xcode build phase 运行 `cargo build` 或预编译 XCFramework。
- 发布时将 Rust static lib/dylib 打包为 `.xcframework`。

---

## 13. 数据库、配置、文件系统

### 13.1 数据目录策略

保留两种模式：

| 模式 | 说明 |
|------|------|
| Compatibility Mode | 用户选择原 LR2 安装目录，直接使用其中 `LR2files/` |
| Native Mode | 使用平台推荐目录保存配置/数据库/资源索引 |

macOS Native：

```text
~/Library/Application Support/OpenLR2/
├── LR2files/
├── Database/
├── Replay/
├── Ghost/
└── Logs/
```

iOS Native：

```text
App Sandbox Documents/
├── Libraries/
├── LR2files/
├── Imports/
├── Database/
└── Replay/
```

### 13.2 SQLite

Rust 使用：

- `rusqlite`，可 bundled sqlite。
- schema migration 层。
- legacy read/write adapter。

### 13.3 配置

保留 legacy XML 读写，同时新增 Rust-native config：

```text
openlr2.toml
```

但任何会影响 LR2 原目录兼容的配置，必须能同步/映射到 legacy XML。

### 13.4 编码

使用：

- `encoding_rs` 处理 CP932/Shift-JIS。
- `camino` 或 UTF-8 path adapter。
- macOS Unicode normalization 审计。

必须测试：

- 日文曲名。
- 中文曲名。
- CP932 文件名。
- 混合大小写资源引用。
- skin 中相对路径。

---

## 14. IR 与 CustomIR 新方案

### 14.1 第一阶段

- Rust runtime 实现内置 IR HTTP client 或先禁用 IR。
- CustomIR 禁用。
- UI 清楚提示“CustomIR plugins are not supported in this build”。

### 14.2 第二阶段：外部进程协议

替代 DLL 插件，推荐外部进程协议：

```text
OpenLR2 runtime
    ↓ JSON-RPC over stdin/stdout 或 local socket
CustomIR helper process
    ↓ HTTP
IR server
```

优点：

- 不受 C++ ABI/Rust ABI/Swift ABI 影响。
- macOS sandbox 更可控。
- 插件崩溃不带崩游戏。
- Linux/macOS/Windows 都能统一。

### 14.3 Score payload

Rust 中定义稳定 schema：

```rust
pub struct IrScorePayload {
    pub schema_version: u32,
    pub chart_hash: String,
    pub title: String,
    pub player_id: String,
    pub score: ScoreState,
    pub gauge: GaugeResult,
    pub options: PlayOptions,
    pub replay_hash: Option<String>,
}
```

序列化：

- JSON for external process。
- MessagePack 可选。

---

## 15. 迁移实施路线

### Phase 0：基线冻结

目标：给 Rust 重构建立不可争辩的行为基准。

任务：

1. 编写 C++ dump 工具或临时命令：
   - dump BMS parse result。
   - dump timing timeline。
   - dump skin SRC/DST/DrawingBuf。
   - dump replay judge result。
2. 固定 fixtures：
   - 7K 基础谱面。
   - BPM/STOP 谱面。
   - LNOBJ 谱面。
   - Mine 谱面。
   - PMS 9K。
   - Base62 keysound。
   - CP932 路径。
   - 默认 LR2 skin。
   - HD skin。
3. 记录 Windows 基准：
   - 选歌截图。
   - 游玩截图。
   - Result 截图。
   - autoplay score。

验收：

- `fixtures/golden/*.json` 可重复生成。
- C++ 当前行为被记录，不再靠记忆迁移。

### Phase 1：Rust workspace 和 BMS parser

目标：Rust 能解析 BMS 并输出与 C++ 基准一致的数据。

任务：

- 建 Rust workspace。
- 实现 encoding/path adapter。
- 实现 BMS lexer/parser。
- 实现 timing timeline。
- 实现 chart hash/resource table。
- 实现 `bms-dump` CLI。

验收：

- `cargo test -p openlr2-bms` 通过。
- `bms-dump fixture.bms` 输出与 C++ golden 匹配。

### Phase 2：Judge/Score/Replay core

目标：Rust 能离线模拟一首歌。

任务：

- 实现 `JudgeEngine`。
- 实现 Gauge/Score。
- 实现 Replay reader/writer。
- 实现 autoplay runner。
- 实现 `replay-verify` CLI。

验收：

- 同一 BMS autoplay 结果与 C++ 一致。
- 同一 replay 结果与 C++ 一致。
- Long note/mine/random/DP/PMS regression 通过。

### Phase 3：Skin parser 和 DrawCommand

目标：Rust 能解析 LR2 skin 并生成 draw commands。

任务：

- 实现 skin CSV parser。
- 迁移 opcode tables。
- 实现 SRC/DST timeline interpolation。
- 实现 option/customize。
- 实现 `skin-dump` CLI。
- 从 C++ DrawingBuf dump 建 golden tests。

验收：

- 默认 select/play/result skin 可解析。
- HD skin 可解析。
- 关键场景给定时间点的 draw command 与 C++ 近似一致。

### Phase 4：Runtime 状态机

目标：Rust 有完整 scene runtime，但可先 headless。

任务：

- 实现 `SceneState`。
- SongSelect：读取 song db，移动 cursor，选择歌曲。
- Decide：准备 chart/resource load。
- Play：tick judge/audio/render commands。
- Result：生成结果状态和保存请求。
- Runtime command/event queue。

验收：

- CLI 可以模拟：启动 -> 选歌 -> autoplay -> result -> 保存。
- 不依赖 Swift、不依赖 GPU。

### Phase 5：macOS Swift 壳原型

目标：macOS app 可以加载 Rust runtime 并显示画面。

任务：

- 建 Xcode/SwiftPM app。
- 集成 `openlr2-ffi` static lib 或 dylib。
- 建 `OpenLR2Runtime` Swift wrapper。
- 建 `MTKView`。
- 实现最小 Metal renderer：
  - clear。
  - image draw。
  - alpha blend。
  - texture upload。
- 传 keyboard/mouse events。

验收：

- macOS app 启动。
- 选择 LR2files 目录。
- 显示 loading/select skin。
- 键盘可以移动选歌。

### Phase 6：音频闭环

目标：macOS 可以 autoplay/手动游玩一首歌。

任务：

- 接 FMOD 或 miniaudio/CoreAudio。
- 实现 sound cache。
- 实现 keysound trigger。
- 实现 BGM/preview。
- 处理 AudioSession/macOS output device。

验收：

- autoplay 一首歌，音画同步可接受。
- 手动输入有按键音。
- Result score 正常。

### Phase 7：macOS 完整可玩 Alpha

目标：日常可玩。

任务：

- 补字体。
- 补 BGA/image layer。
- 补 screenshot。
- 补 replay save/load。
- 补 score db save。
- 补 settings UI。
- 补 log viewer。
- 补 crash safe error reporting。

验收：

- 常用 LR2 皮肤可运行。
- 10 首不同类型 BMS 可玩。
- Score/Replay 兼容。
- 非 ASCII 路径可用。

### Phase 8：iOS 壳

目标：iOS 能跑 runtime，至少 autoplay/外接键盘可玩。

任务：

- 建 UIKit/SwiftUI + MTKView app。
- 集成 Rust XCFramework。
- 文件导入：
  - Files picker。
  - zip 解包可选。
  - 选择 LR2files。
- 输入：
  - 外接键盘。
  - GameController。
  - 触控 overlay 可选。
  - CoreMIDI。
- AudioSession：
  - category。
  - interruption。
  - background/foreground。
- 性能：
  - texture memory。
  - audio latency。
  - battery。

验收：

- iPad 上能打开 app。
- 能导入测试 LR2files。
- 能显示 select/play/result。
- 外接键盘或 autoplay 正常。

### Phase 9：Linux/Windows 壳可选

虽然本方案重点是 Swift Apple 壳，但 Rust 后端应服务 Linux/Windows：

- 用 SDL3/winit + wgpu 做调试壳。
- 复用同一 runtime。
- 与 macOS Swift 壳共享 golden tests。

---

## 16. 仓库演进方案

### 16.1 推荐目录

```text
OpenLR2/
├── LR2/                         # legacy C++ source
├── SkinEditor/
├── lib/
├── rust/
│   ├── Cargo.toml
│   ├── crates/
│   └── tools/
├── apps/
│   ├── macos/
│   ├── ios/
│   └── desktop-sdl/
├── fixtures/
│   ├── bms/
│   ├── skins/
│   ├── replay/
│   └── golden/
├── docs/
│   ├── architecture/
│   ├── migration/
│   └── compatibility/
└── scripts/
    ├── generate-golden/
    ├── build-xcframework/
    └── compare-screenshots/
```

### 16.2 旧 C++ 的角色

旧 C++ 不立刻删除，而是作为：

- 行为参考。
- golden generator。
- 迁移对照。
- Windows baseline。

等 Rust runtime 覆盖对应模块后，再逐步标记 legacy module deprecated。

---

## 17. 测试体系

### 17.1 Rust 单元测试

覆盖：

- Base36/Base62。
- BMS parser。
- BPM/STOP timeline。
- Judge windows。
- Gauge transitions。
- Replay encode/decode。
- Skin DST interpolation。
- SQLite adapter。

### 17.2 Golden tests

```text
C++ legacy output
        ↓
fixtures/golden/*.json
        ↓
Rust output compare
```

比较策略：

- 纯数据：严格一致。
- 浮点 timing：允许极小 epsilon。
- draw command：先比较 command kind/resource/rect/alpha/blend，后期再像素比较。

### 17.3 Snapshot/screenshot tests

macOS app 可做自动 screenshot：

- Select scene after 5 seconds。
- Play scene at 10 seconds autoplay。
- Result scene。

比较：

- 非空。
- 关键区域 hash。
- 可选 perceptual diff。

### 17.4 Fuzz

Rust parser 建议 fuzz：

- BMS parser。
- Skin CSV parser。
- Replay decoder。
- Legacy DB adapter 输入。

使用：

- `cargo-fuzz`
- `arbitrary`

### 17.5 性能测试

指标：

- BMS parse time。
- Skin parse time。
- Per-frame skin evaluate time。
- DrawCommand count。
- Audio trigger latency。
- Memory peak。
- Texture upload time。

---

## 18. 发布与打包

### 18.1 Rust XCFramework

为 Swift app 构建：

```text
OpenLR2Runtime.xcframework
├── macos-arm64
├── macos-x86_64
├── ios-arm64
└── ios-simulator-arm64/x86_64
```

脚本：

```text
scripts/build-xcframework.sh
```

产物：

- static library 或 dynamic library。
- C header。
- modulemap。
- dSYM。

### 18.2 macOS 发布

- `.app` bundle。
- Rust dylib/static lib 嵌入。
- FMOD/miniaudio 等依赖处理。
- `@rpath` 检查。
- codesign。
- notarization。
- Sparkle 更新可选。

### 18.3 iOS 发布

- Rust static lib 链入 app。
- 不使用任意动态插件。
- 资源通过 Documents/import。
- App Store 需要注意：
  - 外部下载执行代码不允许。
  - CustomIR 动态插件不适合。
  - 用户导入 BMS/皮肤资源通常可行，但需要说明版权由用户负责。

---

## 19. 风险与应对

| 风险 | 影响 | 应对 |
|------|------|------|
| 重写范围过大 | 项目长期不可玩 | 先做 headless core 和 golden tests，再做 UI |
| Skin 兼容复杂 | 画面不像 LR2 | DrawCommand golden + screenshot diff |
| 音频延迟 | BMS 游戏体验失败 | 第一阶段 FMOD，后期专项低延迟调优 |
| Swift/Rust FFI 复杂 | 崩溃/内存问题 | C ABI POD，opaque handle，严格 ownership |
| iOS 文件导入复杂 | 用户无法放歌 | 先支持 Documents/LR2files，后续 zip/import UX |
| CustomIR 不兼容 | IR 生态断裂 | 第一阶段禁用，后续外部进程协议 |
| 数据库损坏 | 用户成绩风险 | 默认备份，所有写入事务化 |
| 路径编码 | 资源丢失 | encoding tests + path normalization |
| 过早做 SwiftUI 全 UI | 性能和输入受限 | 游戏画面用 MTKView，SwiftUI 做外围 |

---

## 20. 人力与时间估计

| 阶段 | 工作量 | 主要产出 |
|------|--------|----------|
| Phase 0 | 1-2 周 | golden 基准、fixtures |
| Phase 1 | 2-4 周 | Rust BMS parser |
| Phase 2 | 3-5 周 | Judge/Score/Replay |
| Phase 3 | 4-8 周 | Skin parser + DrawCommand |
| Phase 4 | 3-6 周 | Headless runtime |
| Phase 5 | 2-4 周 | macOS Swift 壳 + Metal 原型 |
| Phase 6 | 2-5 周 | 音频闭环 |
| Phase 7 | 6-12 周 | macOS Alpha |
| Phase 8 | 4-10 周 | iOS Alpha |

最短 macOS 可见原型：约 2-3 个月。

macOS 可玩 Alpha：约 5-8 个月。

iOS 可玩 Alpha：在 macOS Alpha 后约 2-4 个月。

接近 LR2/OpenLR2 高兼容：需要持续积累 fixtures 和用户皮肤/谱面回归，通常是 1 年级别的工程。

---

## 21. 最推荐的第一步

不要先写 Swift UI，也不要先写 renderer。

第一步应做：

1. 写 C++ legacy dump 工具，冻结 BMS/timing/replay/skin 基准。
2. 建 Rust workspace。
3. 实现 `openlr2-bms`。
4. 用 golden tests 证明 Rust parser 与当前 OpenLR2 行为一致。

原因：只要 BMS/timing/judge/skin 语义没有被测试锁住，后面的 Swift 壳和 Metal 渲染都会变成漂亮但不可信的外壳。

最小第一阶段交付物：

```text
rust/crates/openlr2-bms
rust/crates/openlr2-core
rust/tools/bms-dump
fixtures/bms
fixtures/golden/bms
docs/migration/golden-format.md
```

