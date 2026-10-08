# Termino

在终端里玩的俄罗斯方块，用 Rust 编写，支持 macOS、Linux 和 Windows。

```
TTTTTT  ZZZZZZ  LLLL    OO      OO  IIIIII  JJ    JJ  SSSSSS
  TT    ZZ      LL  LL  OOOO  OOOO    II    JJJJ  JJ  SS  SS
  TT    ZZZZ    LLLL    OO  OO  OO    II    JJ  JJJJ  SS  SS
  TT    ZZ      LL  LL  OO      OO    II    JJ    JJ  SS  SS
  TT    ZZZZZZ  LL  LL  OO      OO  IIIIII  JJ    JJ  SSSSSS
```

上图中的字母只是示意，实际显示为彩色方块，每个字母对应一种方块的颜色。

## 特点

- **规则遵循 Tetris Guideline**：SRS 旋转与踢墙、7-bag 随机、暂存（Hold）、落点预览、5 个预览、锁定延迟
- **计分**：单消到四消、T-Spin（含 Mini）、Back-to-Back、连击、软降和硬降得分；每 10 行升一级，下落速度随等级加快
- **长按连发（DAS/ARR）**：终端支持 kitty 键盘协议时由游戏控制，手感更接近原版
- **自动适配终端颜色**：真彩色、256 色、16 色、无颜色四档；也可以只用 ASCII 字符显示
- **可配置**：按键和连发时间都能在配置文件里修改
- **最高分**：自动保存在本地

## 安装

需要 Rust 1.85 或更新版本。

```bash
git clone https://github.com/agiledolphin/termiro.git
cd termiro
cargo install --path crates/termino-tui
termino
```

不安装、直接运行：

```bash
cargo run --release
```

终端窗口至少要 52×22。开始界面的大 Logo 需要 60 列宽，窗口不够宽时显示小号文字。

## 操作

| 按键 | 作用 |
|---|---|
| ← → | 左右移动 |
| ↓ | 软降 |
| Space | 硬降；在开始界面按下开始游戏 |
| ↑ / X | 顺时针旋转 |
| Z | 逆时针旋转 |
| C | 暂存（Hold） |
| P | 暂停，同时显示按键说明 |
| R | 重新开始 |
| Q / Esc | 退出 |
| Ctrl-C | 立即退出 |

- **哪些操作需要确认**：游戏进行中按 R 或 Q/Esc 会先弹出确认框，按 Y 或 Enter 确认，按 N 或 Esc 取消。开始界面按 Q/Esc 和游戏结束后按 R 不需要确认。
- **自动暂停**：终端窗口失去焦点时游戏会自动暂停。

## 终端支持

### 长按连发

游戏需要知道按键何时松开，才能自己控制长按连发，这依赖终端的支持：

| 终端 | 长按连发 |
|---|---|
| Ghostty、kitty、Alacritty、foot | 由游戏控制 |
| WezTerm、iTerm2 | 需要在终端设置里开启 kitty 键盘协议 |
| Windows（各种终端） | 由游戏控制，因为系统本身会上报按键松开 |
| macOS 自带「终端」、tmux | 由系统的按键重复控制 |

按 P 暂停，最后一行会显示当前模式：`auto-repeat: DAS` 表示由游戏控制；`auto-repeat: OS` 表示由系统控制，这时连发速度取决于系统的键盘设置。

### 颜色

默认会根据 `NO_COLOR`、`COLORTERM`、`TERM` 环境变量自动选择颜色档位：

| 档位 | 典型环境 |
|---|---|
| 真彩色 | iTerm2、Ghostty、kitty、WezTerm、Windows Terminal |
| 256 色 | macOS 自带「终端」，大多数 Linux 终端 |
| 16 色 | 老式终端、Windows 旧控制台 |
| 无颜色 | 设置了 `NO_COLOR`，或 `TERM=dumb`；方块显示为 `[]` |

如果边框显示错位（常见于把方框字符当作双宽字符的中日韩语言环境），可以在配置中打开 `ascii`，边框改用 `+ - |`。

## 配置

配置文件是可选的，不存在时使用默认值。查看配置文件的位置，以及输出带注释的默认配置：

```bash
termino --config-path      # 例如 macOS：~/Library/Application Support/termino/config.toml
termino --default-config

# 生成一份默认配置，在此基础上修改
mkdir -p "$(dirname "$(termino --config-path)")"
termino --default-config > "$(termino --config-path)"
```

配置文件里的每一项都可以省略，省略的项使用默认值：

```toml
[timing]
das_ms = 167        # 按住方向键多久后开始连续移动
arr_ms = 33         # 连续移动的间隔，0 表示瞬间到墙
soft_drop_ms = 33   # 按住软降时每下落一行的间隔，0 表示直接到底

[keys]
move_left = ["Left"]
move_right = ["Right"]
soft_drop = ["Down"]
hard_drop = ["Space"]
rotate_cw = ["Up", "x"]
rotate_ccw = ["z"]
hold = ["c"]
pause = ["p"]
restart = ["r"]
quit = ["q", "Esc"]

[display]
color = "auto"      # auto / truecolor / 256 / 16 / none
ascii = false       # 只用 ASCII 字符画边框
```

- **按键名**：可以用 `Left`、`Right`、`Up`、`Down`、`Space`、`Enter`、`Tab`、`Backspace`、`Esc`，或任意单个字符，字母不区分大小写。
- **出错时**：配置有误（字段名写错、按键名无法识别、同一个键绑定了两个操作）时，程序会在启动前报错并退出。

## 最高分

最高分保存在：

- macOS：`~/Library/Application Support/termino/highscore.toml`
- Linux：`~/.local/share/termino/highscore.toml`
- Windows：`%APPDATA%` 下的 `termino` 目录

游戏结束、中途重开或退出时，都会结算当前这一局。删除这个文件即可清空纪录。

## 开发

```bash
cargo test --workspace                                # 运行全部测试
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

### 项目结构

```
crates/
├── termino-core/   # 游戏逻辑：零 IO、确定性，同样的种子和输入一定得到同样的结果
│   ├── board.rs        盘面与消行
│   ├── piece.rs        方块、朝向
│   ├── rotation.rs     SRS 踢墙表
│   ├── randomizer.rs   7-bag 随机器（自带伪随机数生成器，保证回放可复现）
│   ├── scoring.rs      计分、等级、重力曲线
│   └── game.rs         Game::update(dt, actions) -> Vec<Event>
└── termino-tui/    # 终端界面
    ├── main.rs         命令行、主循环（60Hz 固定步长）
    ├── platform.rs     终端初始化与还原、能力探测
    ├── input.rs        按键处理、DAS/ARR
    ├── keymap.rs       按键配置解析
    ├── config.rs       配置文件
    ├── storage.rs      最高分存档
    ├── app.rs          界面状态：开始界面、暂停、确认框
    └── render/         绘制
```

### 测试

- **core**：除了单元测试，还用 proptest 对随机操作序列检查不变量，例如方块不会重叠、格子数守恒、同样的种子和输入得到同样的结果。
- **界面**：用 insta 做快照测试，快照里有背景色的格子会换成对应方块的字母。修改界面后用下面的命令更新快照，再用 git diff 检查改动：

  ```bash
  INSTA_UPDATE=always cargo test -p termino-tui
  ```

- **CI**：每次推送都会在 macOS、Linux、Windows 上运行格式检查、clippy 和全部测试。
