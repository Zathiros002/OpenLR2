# OpenLR2 Linux/macOS 移植方案

> 目标：在不破坏当前 Windows x86/VS2022 主线的前提下，把 OpenLR2 移植到 Linux x86_64/aarch64 和 macOS arm64/x86_64。
>
> 当前结论：这不是简单改 CMake 的工作。项目目前依赖 Windows 版 DxLib、Windows 版 FMOD、Win32/WinMM API、Windows DLL 插件 ABI 和 LR2 原始资源目录模型。推荐先完成平台边界拆分，再逐步替换渲染、音频、输入、文件和插件后端。

---

## 1. 当前阻塞点

### 1.1 构建层

- `CMakeLists.txt` 只在 `WIN32` 下配置 FMOD 和 DxLib；非 Windows 会触发 `message(FATAL_ERROR "No FMOD for this platform")` 和 `message(FATAL_ERROR "No DxLib for this platform")`。
- `CMakePresets.json` 当前只有 Visual Studio 17 2022 + Win32 预设。
- `SkinEditor` 顶层构建只在 `WIN32` 下启用，且使用仓库内 `SkinEditor/SDL3.lib`。
- 源码中存在一些非 Windows stub 和 `DxLib-for-Linux` 注释，但未形成完整、可构建的 Linux/macOS 目标。

### 1.2 运行时依赖

| 模块 | 当前状态 | Linux/macOS 风险 |
|------|----------|------------------|
| 图形/窗口 | DxLib Windows `.lib`，大量代码直接调用 DxLib API | Linux/macOS 无法直接链接；需要 DxLib-for-Linux 或新渲染后端 |
| 音频 | FMOD 头文件 + Windows x86/x64 DLL/import lib | 需要平台 SDK/许可/打包；或替换为 SDL audio/miniaudio/OpenAL 等 |
| 输入 | DxLib 键盘/手柄 + Windows WinMM MIDI | MIDI、手柄、键盘布局、文本输入需要平台后端 |
| 文件系统 | 混合 `std::filesystem`、Win32 文件 API、LR2 相对目录 | macOS bundle、Linux XDG、大小写、Unicode 规范化都要处理 |
| 插件 | CustomIR 使用 Windows DLL + `LoadLibraryW`/`GetProcAddress` | Linux `.so`、macOS `.dylib` ABI 需另建；macOS 签名/加载限制更多 |
| 录像/导出 | `En_recordmovie` 中有 Windows API 和旧 AVI/压缩设置 | 桌面跨平台建议改为 FFmpeg/libav 或禁用第一阶段功能 |
| SkinEditor | SDL3/ImGui，但仍包含 DxLib/Windows 入口依赖 | 需要先拆掉 DxLib 初始化和 Windows include |

---

## 2. 总体策略

采用“兼容层优先，功能逐步替换”的路线。

1. **建立平台接口层**：先把散落在 `Main.cpp`、`En_*`、场景和 SkinEditor 中的平台调用收口。
2. **保留 Windows 行为基准**：所有抽象先由 Windows 后端实现，确保原行为不变。
3. **先跑 Linux 桌面**：Linux 的调试/CI/依赖安装更开放，适合作为首个非 Windows 目标。
4. **再跑 macOS**：macOS 需要额外处理 bundle、签名、CoreMIDI、Metal/OpenGL 可用性和沙盒路径。
5. **分阶段交付**：先实现“能启动、能选歌、能播放、能保存分数”，再补齐 IR、CustomIR、录像、SkinEditor。

---

## 3. 目标平台与构建矩阵

### 3.1 第一批目标

| 平台 | 编译器 | 架构 | 目标 |
|------|--------|------|------|
| Linux | Clang/GCC | x86_64 | 首个可运行非 Windows 桌面版 |
| Linux | Clang/GCC | aarch64 | Raspberry Pi/ARM Linux 后续验证 |
| macOS | AppleClang | arm64 | Apple Silicon 原生版 |
| macOS | AppleClang | x86_64 | Intel macOS 可选支持 |

### 3.2 CMake 预设建议

新增预设：

- `linux-clang-debug`
- `linux-clang-relwithdebinfo`
- `macos-arm64-debug`
- `macos-arm64-relwithdebinfo`
- `macos-universal-relwithdebinfo`，可选，使用 `CMAKE_OSX_ARCHITECTURES=arm64;x86_64`

### 3.3 构建开关建议

```cmake
option(OPENLR2_ENABLE_FMOD "Use FMOD audio backend" ON)
option(OPENLR2_ENABLE_CUSTOM_IR "Enable CustomIR dynamic plugins" ON)
option(OPENLR2_ENABLE_MOVIE_EXPORT "Enable movie export" OFF)
option(OPENLR2_ENABLE_SKIN_EDITOR "Build SkinEditor" OFF)
option(OPENLR2_USE_DXLIB_COMPAT "Use DxLib compatibility backend" ON)
```

第一阶段建议默认：

| 平台 | FMOD | CustomIR | Movie Export | SkinEditor |
|------|------|----------|--------------|------------|
| Windows | ON | ON | ON | ON |
| Linux | ON 或 OFF | OFF | OFF | OFF |
| macOS | ON 或 OFF | OFF | OFF | OFF |

---

## 4. 架构改造方案

### 4.1 新增平台目录

建议新增：

```text
LR2/platform/
├── Platform.h
├── Platform.cpp
├── FileSystem.h
├── FileSystem.cpp
├── DynamicLibrary.h
├── MessageBox.h
├── Time.h
├── Midi.h
├── Window.h
├── Input.h
└── backends/
    ├── win32/
    ├── linux/
    ├── macos/
    └── sdl3/
```

### 4.2 平台接口职责

| 接口 | 替换对象 | 说明 |
|------|----------|------|
| `platform::GetExecutableDirectory()` | `GetModuleFileNameW`、`/proc/self/exe` | Linux/macOS 分别处理 `/proc`、`_NSGetExecutablePath`、bundle |
| `platform::ShowMessage()` | `MessageBoxA` 和非 Windows console stub | 第一阶段可日志输出；macOS 后续可 Cocoa alert |
| `platform::CopyFileIfMissing()` | `CopyFile` | 使用 `std::filesystem::copy_file` |
| `platform::DynamicLibrary` | `LoadLibraryW/GetProcAddress/FreeLibrary` | Linux `dlopen`，macOS `dlopen` |
| `platform::MidiInput` | WinMM MIDI | Linux ALSA/JACK/PipeWire，macOS CoreMIDI |
| `platform::HighResolutionTimer` | DxLib/Win32 timer 混合 | 封装 `std::chrono` 或平台高精度时钟 |
| `platform::UserDataDirectory()` | `LR2files/` 相对路径 | 第一阶段仍支持相对目录，后续可迁移到 XDG/Application Support |

### 4.3 渲染/窗口后端

有两条路线：

#### 路线 A：DxLib 兼容路线

目标是最小改动，通过 DxLib-for-Linux 或自建 DxLib 兼容层让现有调用继续工作。

优点：

- 初期改动较少。
- `LR2_skindraw`、`LR2_skinobject`、BGA、字体等大量 DxLib 调用可以暂时不重写。

缺点：

- macOS 支持不确定。
- 长期会被 DxLib API 绑定，移动端和现代图形后端困难。
- 仍需要处理字体、视频、输入、窗口行为差异。

适合：快速验证 Linux 可运行性。

#### 路线 B：SDL3 + 渲染后端路线

目标是把 OpenLR2 主程序迁到 SDL3 窗口/输入/渲染体系，SkinEditor 已经携带 SDL3 源码可作为参考。

建议后端：

- 窗口/输入：SDL3。
- 图像加载：stb_image 或 SDL_image。
- 2D 渲染：SDL_Renderer、OpenGL、Metal、Vulkan 或 bgfx。
- 字体：FreeType + HarfBuzz，或 SDL_ttf。
- 视频/BGA：FFmpeg/libav 或平台解码器。

优点：

- Linux/macOS 长期维护更可控。
- 可自然支持 macOS arm64。
- 后续 Android/iOS 路线更清晰。

缺点：

- 初期工作量大。
- 需要重建 DxLib 绘制语义：纹理句柄、BlendMode、MakeScreen、DrawGraph、DrawRotaGraph、字体、RenderTarget 等。

适合：长期主线。

推荐策略：先做一个 **DxLibCompat 接口层**，接口保持接近当前使用方式，底层第一阶段仍接 DxLib/Windows，Linux/macOS 后续可接 SDL3/OpenGL/Metal。不要在业务层直接引入 SDL3 API。

---

## 5. 模块级任务拆分

### 5.1 CMake 和依赖管理

任务：

1. 把 FMOD 和 DxLib 的 `FATAL_ERROR` 改成按后端选择。
2. 将平台源文件用 generator expression 或条件 target 管理。
3. 引入 `OpenLR2Platform` 静态库。
4. 引入 `OpenLR2BackendAudio`、`OpenLR2BackendGraphic`、`OpenLR2BackendInput` 目标。
5. Linux/macOS 默认先不构建 SkinEditor 和电影导出。

验收：

- Windows 现有 VS/CMake 构建不退化。
- Linux/macOS 能完成 CMake configure。
- `OpenLR2Lib` 不再无条件链接 Windows-only DxLib/FMDO import lib。

### 5.2 图形/窗口

当前关键调用集中在：

- `LR2/Main.cpp`
- `LR2/En_graphic.cpp`
- `LR2/LR2_skindraw.cpp`
- `LR2/LR2_skinload.cpp`
- `LR2/LR2_skinobject.cpp`
- `LR2/Scene*.cpp`

第一阶段最小目标：

- 启动窗口 640x480。
- 支持窗口/全屏切换。
- 支持加载 PNG/BMP/JPG。
- 支持 render target，对应 DxLib `MakeScreen`/`SetDrawScreen`。
- 支持 alpha blend、add blend、颜色调制、缩放、旋转。
- 支持字体绘制和 UTF-8。
- 支持截图。

需要设计的兼容句柄：

```cpp
using TextureHandle = int;
using RenderTargetHandle = int;
using FontHandle = int;
```

内部用资源表映射到底层纹理/字体对象，避免一次性改动所有 `int GrHandle[]`。

风险点：

- LR2 皮肤依赖大量像素级定位，渲染差一两个像素都可能明显。
- DxLib 的 blend/颜色/坐标取整语义需要用截图回归。
- macOS Retina/high-DPI 需要固定逻辑分辨率和 drawable scale。

验收：

- 可显示 loading 图。
- 可显示歌曲选择皮肤。
- 基础 LR2 皮肤元素位置与 Windows 截图对齐。
- 游玩场景 Note、判定线、判定文字、血条可正常绘制。

### 5.3 音频

当前结构中 `AUDIO`、`SOUNDDATA` 直接保存 FMOD 指针。短期可保留 FMOD 作为第一后端，但要把 FMOD 类型从公共结构中隔离。

建议步骤：

1. 新增 `AudioBackend` 接口：

```cpp
class AudioBackend {
public:
    virtual bool Init(const AudioConfig&) = 0;
    virtual SoundId LoadSound(std::string_view path, SoundLoadFlags flags) = 0;
    virtual ChannelId Play(SoundId sound, ChannelGroup group, PlayParams params) = 0;
    virtual void Stop(ChannelId channel) = 0;
    virtual void SetVolume(ChannelId channel, float volume) = 0;
    virtual void SetPitch(ChannelId channel, float pitch) = 0;
    virtual void Update() = 0;
};
```

2. 将 `FMOD_SOUND*`、`FMOD_CHANNEL*` 替换为 opaque id 或后端私有数据。
3. Windows 先实现 `FmodAudioBackend`。
4. Linux/macOS 选择：
   - 继续使用 FMOD：引入官方 Linux/macOS SDK。
   - 或使用 miniaudio/SDL audio：重做多音并发、低延迟和 pitch。

推荐：第一阶段继续 FMOD，减少变量；等 Linux/macOS 基础可运行后再评估开源替代。

关键指标：

- 按键音延迟。
- 大量 keysound 并发。
- BGM/keysound 分组音量。
- pitch/speed 变更。
- preview 淡入淡出。

验收：

- 同一谱面在 Windows/Linux/macOS 播放节奏一致。
- 7K/14K 高密度谱面无明显爆音和掉音。
- 音量、预览、暂停/恢复正常。

### 5.4 输入和 MIDI

键盘/手柄建议统一迁到 SDL3 输入事件。

MIDI 分平台：

| 平台 | 后端 |
|------|------|
| Windows | 现有 WinMM，后续可迁到 RtMidi |
| Linux | ALSA sequencer 或 RtMidi |
| macOS | CoreMIDI 或 RtMidi |

推荐使用 RtMidi 做薄封装，减少平台分支。

任务：

1. 保留现有 `inputStructure` 和按键映射逻辑。
2. 将物理输入采集改为 `InputBackend` 填充 `inputStructure`。
3. 将 `midi.input[]` 写入逻辑改为后端回调。
4. 统一键码表，建立 DxLib key id 到 SDL scancode 的映射。

验收：

- 5K/7K/9K 键位配置可读写。
- 键盘长按、快速连打、Start/Select 组合正常。
- 手柄/控制器基础输入正常。
- MIDI note on/off 和 pedal 正常。

### 5.5 文件系统与路径

第一阶段保持原 LR2 目录兼容：

```text
OpenLR2 executable directory/
└── LR2files/
    ├── Config/
    ├── Database/
    ├── Replay/
    ├── Ghost/
    └── CustomIRs/
```

Linux/macOS 后续建议：

| 数据类型 | Linux | macOS |
|----------|-------|-------|
| 可执行资源 | AppDir 或安装目录 | `.app/Contents/Resources` |
| 用户配置 | `$XDG_CONFIG_HOME/OpenLR2` | `~/Library/Application Support/OpenLR2` |
| 数据库/Replay | `$XDG_DATA_HOME/OpenLR2` | `~/Library/Application Support/OpenLR2` |
| 日志 | `$XDG_STATE_HOME/OpenLR2` | `~/Library/Logs/OpenLR2` |

必须审计：

- 路径大小写：Linux/macOS 大小写行为可能不同。
- Unicode：macOS 文件名常见 NFD，Windows 常见 UTF-16，BMS/皮肤可能混 CP932。
- 路径分隔符：继续使用 `std::filesystem::path` 和 `fs::make_preferred`。
- 相对路径：从 executable dir 切到 resource dir/user data dir 后，所有 `LR2files/...` 都要经 path service。

验收：

- 能从原版 LR2 目录直接运行兼容模式。
- 能在平台用户目录创建新配置。
- 歌曲扫描、皮肤加载、Replay/Ghost/Score 保存正常。

### 5.6 数据库和配置

SQLite/TinyXML/MD5 都是源码集成，跨平台风险较低。

任务：

1. 确认 SQLite 编译选项在 Linux/macOS 下无 MSVC 假设。
2. 给数据库访问加最小回归测试：
   - 读取 song.db。
   - 读取玩家 score db。
   - 保存一条成绩。
   - 读取 Ghost/Replay 元数据。
3. 审计 `g_db_lock` 覆盖范围，避免后台 banner/preview/IR 与主线程读写冲突。

验收：

- 原 Windows 生成的数据库可被 Linux/macOS 读取。
- Linux/macOS 生成的成绩数据库回到 Windows 可读。
- 非 ASCII 曲名、路径、艺术家名可查询和显示。

### 5.7 CustomIR

第一阶段建议 Linux/macOS 默认关闭 CustomIR。原因是当前接口绑定 Windows DLL 文件名和加载方式。

桌面跨平台可选方案：

| 方案 | 说明 | 推荐度 |
|------|------|--------|
| 暂时关闭 | Linux/macOS 可玩但不加载插件 | 第一阶段推荐 |
| 多平台动态库 | Windows `.dll`、Linux `.so`、macOS `.dylib` | 第二阶段可做 |
| 外部进程协议 | 插件作为独立进程，通过 stdin/stdout 或 HTTP 通信 | 长期更安全 |
| 内置网络 API | 常用 IR 做成内置模块 | 适合发布版 |

若保留动态库，需要：

- 定义 C ABI，避免 C++ ABI 跨编译器差异。
- 定义命名规则：`.linux-x64.so`、`.linux-arm64.so`、`.macos-arm64.dylib` 等。
- macOS 处理签名、公证、quarantine 和 hardened runtime。

验收：

- 关闭 CustomIR 时游戏流程不受影响。
- 插件加载失败只记录日志，不阻断启动。
- 第二阶段至少有一个 Linux/macOS 示例插件可登录/发送模拟成绩。

### 5.8 IR 网络

`LR2_ir.cpp` 需要单独审计网络实现。如果依赖 DxLib/WinINet/Windows socket，需要替换为跨平台 HTTP 客户端。

建议：

- 统一到 libcurl 或 asio/beast。
- 网络请求从游戏主线程剥离。
- 为登录、榜单、发送成绩建立 mock 测试。

验收：

- 网络不可用时不阻塞游戏。
- 登录失败、超时、服务器错误都有明确日志。
- Linux/macOS 与 Windows 请求内容一致。

### 5.9 录像/导出

第一阶段关闭：

- `-auto2avi`
- `-replay2avi`
- `-bga2avi`
- `-movie`

后续替代方案：

- 视频编码：FFmpeg/libav。
- 音频混流：FFmpeg/libav 或后端离线混音。
- 压缩设置 UI：改为跨平台配置文件，不使用 Windows codec dialog。

验收：

- 禁用时命令行给出明确错误或日志。
- 不影响普通游玩和 Replay。

### 5.10 SkinEditor

SkinEditor 已有 SDL3/ImGui，但当前仍包含：

- `#include <windows.h>`
- `#include <DxLib/DxLib.h>`
- `SetUseDirect3DVersion`
- `DxLib_Init/DxLib_End`
- `SkinEditor/SDL3.lib`

移植步骤：

1. 移除 SkinEditor 对 DxLib 的启动依赖。
2. 使用 SDL3 CMake target，而不是 Windows `.lib`。
3. 用 SDL renderer/OpenGL/Metal 统一图片预览。
4. 把 SkinEditor 作为独立 target，不强制链接完整 `OpenLR2Lib`，只链接皮肤解析/模型所需模块。

优先级：低于主游戏 Linux/macOS 可运行。

---

## 6. 分阶段里程碑

### Phase 0：基线和审计

目标：不改行为，建立事实清单。

任务：

- Windows x86 Release 成功构建并记录构建步骤。
- 记录一套固定测试资源：1 个 LR2 基础目录、3-5 张 BMS、1 套默认皮肤、1 套 HD 皮肤、1 个 Replay。
- 采集 Windows 基准截图：
  - 启动画面。
  - 选歌画面。
  - 决定画面。
  - 游玩 10 秒。
  - Result。
- 采集音频/判定基准：
  - 自动播放一首谱面。
  - 记录 EXScore、判定数、最大 combo。

交付物：

- `docs/porting/baseline.md`
- 基准截图和日志。

### Phase 1：CMake 可配置

目标：Linux/macOS 可以 configure，哪怕暂时不链接完整游戏。

任务：

- 将 Windows-only 依赖改为可选后端。
- 新增 `OpenLR2Platform` target。
- 新增 Linux/macOS CMake presets。
- 给不可用功能加 `#if` 和明确 stub。

验收：

- `cmake --preset linux-clang-debug` 成功。
- `cmake --preset macos-arm64-debug` 成功。
- Windows preset 仍成功。

### Phase 2：Headless/Core 编译

目标：非图形核心模块在 Linux/macOS 编译通过。

包含：

- `strclass`
- `En_fileutil` 中非图形部分
- `En_dbio`
- `En_xml`
- `LR2_configsave`
- `LR2_bmsload` 中非 DxLib 加载部分
- SQLite/TinyXML/MD5

任务：

- 拆分 `OpenLR2Core` 和 `OpenLR2Runtime`。
- 去掉核心模块对 `DxLib/DxLib.h` 的无条件包含。
- 为 BMS parser、config、db 增加小测试。

验收：

- Linux/macOS 可编译并运行 core tests。
- 能解析 BMS 元信息。
- 能读写配置 XML。
- 能打开 SQLite score/song db。

### Phase 3：窗口和基础渲染

目标：Linux/macOS 打开窗口，显示静态皮肤画面。

任务：

- 实现 SDL3 窗口后端。
- 实现纹理加载、render target、基础 draw API。
- 将 `En_graphic` 迁到兼容接口。
- 暂时禁用视频/BGA 和复杂字体功能，先显示图片。

验收：

- Linux/macOS 可启动到 640x480 窗口。
- 可显示 loading.bmp。
- 可进入歌曲选择，皮肤图片能显示。
- 截图与 Windows 基准大体对齐。

### Phase 4：输入和主循环

目标：可以用键盘在选歌界面操作。

任务：

- SDL3 keyboard/gamepad backend。
- 键码映射到现有 `inputStructure`。
- 替换 `ProcessMessage`/窗口关闭事件。
- 实现全屏/窗口切换。

验收：

- 选歌上下移动。
- 进入决定/返回。
- ESC/退出正常。
- macOS Cmd+Q/window close 不崩溃。

### Phase 5：音频和游玩

目标：能完整游玩一首 BMS。

任务：

- 接入 FMOD Linux/macOS SDK 或临时音频后端。
- 实现 keysound 加载、BGM 播放、channel group、volume。
- 实现 `FMOD_System_Update` 等价更新。
- 确认线程加载 keysound 的并发安全。

验收：

- 可从选歌进入游玩。
- 自动播放一首谱面，判定/EXScore 与 Windows 基准一致。
- 手动输入可打键。
- Result 可显示并返回。

### Phase 6：平台细节补齐

目标：达到日常可玩。

任务：

- 字体/UTF-8/CP932 路径稳定。
- Preview、BGA 图片、Banner、Stagefile 正常。
- Replay 保存/回放。
- Score 保存。
- Screenshot。
- 低延迟音频调优。
- MIDI 后端。

验收：

- 常见 LR2 皮肤可用。
- 非 ASCII 曲名和路径正常。
- Replay 可跨平台回放。
- Score db 不损坏。

### Phase 7：发布打包

Linux：

- AppImage 或 tarball。
- 可选 Flatpak。
- 依赖策略：内置动态库或系统包。
- 桌面文件、图标、MIME 可选。

macOS：

- `.app` bundle。
- `Contents/Resources` 放默认资源。
- `@rpath`/`install_name_tool` 修正动态库路径。
- codesign。
- notarization，可选。
- 处理 quarantine 文档说明。

验收：

- 干净机器可运行。
- 日志路径明确。
- 找不到 `LR2files` 时提示清晰。

---

## 7. 推荐技术选型

### 7.1 首选组合

| 领域 | 推荐 |
|------|------|
| 窗口/输入 | SDL3 |
| 2D 渲染 | SDL3 GPU/OpenGL 起步，macOS 后续 Metal |
| 音频 | 第一阶段 FMOD，多平台 SDK；第二阶段评估 miniaudio |
| MIDI | RtMidi |
| 图片 | stb_image 或 SDL_image |
| 字体 | FreeType + HarfBuzz，或 SDL_ttf 起步 |
| 视频/BGA | FFmpeg/libav |
| 网络 | libcurl |
| 打包 | Linux AppImage/tarball，macOS `.app` + codesign |

### 7.2 不建议第一阶段做的事

- 一次性重写所有皮肤绘制逻辑。
- 一边移植一边重构 `game` 大结构。
- 第一阶段就支持 CustomIR 动态插件。
- 第一阶段就支持录像导出。
- 第一阶段就追求移动端代码复用。

---

## 8. 风险清单

| 风险 | 影响 | 缓解 |
|------|------|------|
| DxLib 绘制语义难复刻 | 皮肤位置/透明度/混合效果错误 | 建截图回归，逐 API 对齐 |
| 音频延迟不达标 | BMS 游玩体验不可接受 | 优先保留 FMOD，多平台低延迟调参 |
| Unicode/CP932 路径问题 | 曲库扫描失败 | 建含日文/中文/特殊符号路径测试集 |
| 大小写敏感路径 | Linux 找不到资源 | 路径规范化和资源查找 fallback |
| macOS Retina 缩放 | 画面模糊或坐标错位 | 固定逻辑分辨率，分离 window size/drawable size |
| 插件 ABI 不稳定 | CustomIR 崩溃 | 第一阶段禁用，后续 C ABI/进程隔离 |
| 数据库写入损坏 | 成绩丢失 | 默认备份，测试跨平台读写 |
| 后台线程访问图形资源 | 跨平台崩溃 | 资源创建回主线程，后台只做 IO/解码 |

---

## 9. 验收测试矩阵

### 9.1 功能测试

| 功能 | Linux | macOS |
|------|-------|-------|
| 启动并读取配置 | 必须 | 必须 |
| 选歌列表 | 必须 | 必须 |
| 皮肤图片绘制 | 必须 | 必须 |
| 键盘输入 | 必须 | 必须 |
| BMS 加载 | 必须 | 必须 |
| 按键音播放 | 必须 | 必须 |
| 成绩保存 | 必须 | 必须 |
| Replay | 第二阶段 | 第二阶段 |
| MIDI | 第二阶段 | 第二阶段 |
| CustomIR | 第三阶段 | 第三阶段 |
| 录像导出 | 第三阶段 | 第三阶段 |

### 9.2 回归资源

至少准备：

- 普通 7K BMS。
- 5K BMS。
- 9K PMS。
- 长音符谱面。
- 高密度 keysound 谱面。
- 带 BGA/BMP/stagefile/banner 的谱面。
- CP932 文件名/路径谱面。
- UTF-8 文件名/路径谱面。
- HD skin。
- 默认 LR2 skin。

### 9.3 自动化测试建议

- Core 单元测试：BMS parse、配置读写、SQLite 读写、MD5。
- Golden data 测试：同一 BMS 输出相同 note count、BPM timeline、hash。
- Screenshot smoke test：固定输入进入选歌和游玩场景后截图，检查非空和关键区域像素。
- Audio smoke test：加载 N 个 keysound 并发播放，检查无异常返回。

---

## 10. 建议落地顺序

1. 建 `OpenLR2Platform`，先迁移 `GetExecutablePath`、`MessageBoxA`、`CopyFile`、动态库加载。
2. 调整 CMake，让 Linux/macOS 能 configure，禁用 FMOD/DxLib fatal path。
3. 拆 `OpenLR2Core`，让 BMS/config/db 在 Linux/macOS 编译测试。
4. 做 SDL3 窗口和最小图形兼容层。
5. 接入音频后端，完成自动播放一首歌。
6. 补键盘/手柄/MIDI。
7. 补路径、字体、BGA、Replay、Score 保存。
8. 再考虑 SkinEditor、CustomIR、录像导出。

---

## 11. 粗略工作量估计

| 阶段 | 经验工程师工作量 | 说明 |
|------|------------------|------|
| Phase 0-1 | 1-2 周 | 构建、审计、平台接口雏形 |
| Phase 2 | 2-4 周 | Core 拆分和测试 |
| Phase 3 | 4-8 周 | 图形兼容层初版 |
| Phase 4 | 1-2 周 | 输入和窗口事件 |
| Phase 5 | 2-4 周 | 音频和游玩闭环 |
| Phase 6 | 4-10 周 | 稳定性、路径、字体、Replay、MIDI |
| Phase 7 | 1-3 周 | 打包发布 |

最短可玩原型：约 2-3 个月。

接近可公开测试版：约 4-6 个月。

质量接近 Windows 主线：取决于皮肤兼容、音频延迟、BGA/视频和长尾资源问题，通常需要持续迭代。

