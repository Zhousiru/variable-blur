<div align="center">

# variable-blur

**Apple 风格可变模糊 — Node.js 原生模块**

<br>

<img src="docs/example-blurred.png" width="720">

<br>
<br>

[English](./README.md)

</div>

## Installation

```bash
npm install variable-blur
```

预编译二进制，支持 macOS / Linux / Windows（x64 & arm64）。

## Quick Start

```js
import sharp from 'sharp'
import { variableBlur } from 'variable-blur'

const input = await sharp('photo.jpg').png().toBuffer()

const output = variableBlur({
  buffer: input,
  options: {
    preset: 'balanced',
    maxSigma: 32,
  },
})

await sharp(output).toFile('photo-blurred.png')
```

## Debug UI

```bash
cargo run -p debug-ui --release
```

内置 [egui](https://github.com/emilk/egui) 桌面工具，可实时调节参数。

## API

### `variableBlur(input): Buffer`

| 参数            | 类型      | 说明                               |
| :-------------- | :-------- | :--------------------------------- |
| `input.buffer`  | `Buffer`  | 编码后的图片（PNG、JPEG、WebP 等） |
| `input.options` | `object?` | 见下方                             |

### Options

| 字段           | 类型     | 可选 | 默认值       | 说明                                                         |
| :------------- | :------- | :--: | :----------- | :----------------------------------------------------------- |
| `x`            | `number` | 是   | `1`          | 模糊方向 X 分量                                              |
| `y`            | `number` | 是   | `0`          | 模糊方向 Y 分量                                              |
| `start`        | `number` | 是   | 自动         | 模糊起始投影坐标                                             |
| `end`          | `number` | 是   | 自动         | 模糊达到最大值的投影坐标                                     |
| `preset`       | `string` | 是   | `"balanced"` | `"fast"` / `"balanced"` / `"high"`                           |
| `maxSigma`     | `number` | 是   | 取决于预设   | 最大 sigma（`fast`=24, `balanced`=32, `high`=40）            |
| `curve`        | `string` | 是   | `"power"`    | `"linear"`、`"power(γ)"`、`"cubic-bezier(x1,y1,x2,y2)"`     |
| `schedule`     | `string` | 是   | `"power"`    | `"linear"`、`"power(γ)"`                                     |
| `outputFormat` | `string` | 是   | 与输入相同   | `"png"` / `"jpeg"` / `"webp"` / `"bmp"` / `"tiff"` / `"tga"` |
| `advanced`     | `object` | 是   | &mdash;      | 见 [Advanced Options](#advanced-options)                     |

### Advanced Options

<details>
<summary>底层金字塔配置（通常无需修改）</summary>

<br>

| 字段                            | 类型     | 默认值     | 说明                   |
| :------------------------------ | :------- | :--------- | :--------------------- |
| `advanced.mode`                 | `string` | `"auto"`   | `"auto"` 或 `"manual"` |
| `advanced.steps`                | `number` | 取决于预设 | 离散模糊级别数         |
| `advanced.maxLevels`            | `number` | 取决于预设 | 最大下采样深度         |
| `advanced.targetLocalSigma`     | `number` | 取决于预设 | 每级目标 local sigma   |
| `advanced.minLocalSigma`        | `number` | 取决于预设 | 每级最小 local sigma   |
| `advanced.maxLocalSigma`        | `number` | 取决于预设 | 每级最大 local sigma   |
| `advanced.downsampleStageSigma` | `number` | `0.5`      | 2x 下采样前等效 sigma  |

| 预设       | steps | maxLevels | targetLocalSigma | minLocalSigma | maxLocalSigma |
| :--------- | :---: | :-------: | :--------------: | :-----------: | :-----------: |
| `fast`     |   7   |     6     |       1.6        |      0.3      |      3.0      |
| `balanced` |  10   |     4     |       2.0        |      0.5      |      4.0      |
| `high`     |  14   |     2     |       2.4        |      0.8      |      5.0      |

</details>

## 许可证

Apache-2.0
