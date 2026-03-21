<div align="center">

# Variable Blur

**制作 iOS 风味可变模糊的 Node.js 原生模块**

[![NPM Version](https://img.shields.io/npm/v/variable-blur?style=flat-square)](https://www.npmjs.com/package/variable-blur)

<img src="docs/example-blurred.png" width="500">

</div>

## Installation

```bash
npm install variable-blur
```

预编译二进制，支持 macOS / Linux / Windows（x64 & arm64）。

## Quick Start

```js
import { readFile, writeFile } from 'node:fs/promises'
import { variableBlur } from 'variable-blur'

const input = await readFile('photo.jpg')

const output = variableBlur({
  buffer: input,
  options: {
    x: 1,
    y: 0,
    maxSigma: 32,
    quality: 0.5,
  },
})

await writeFile('photo-blurred.jpg', output)
```

`variableBlur` 的输入和输出都是编码后的图片字节流。

## 与 Sharp 集成

```js
import sharp from 'sharp'
import { variableBlurRaw } from 'variable-blur'

const pipeline = sharp('photo.jpg').resize(1400).ensureAlpha()
const { data, info } = await pipeline.raw().toBuffer({ resolveWithObject: true })

const blurred = variableBlurRaw({
  data,
  width: info.width,
  height: info.height,
  channels: info.channels,
  options: {
    x: 1,
    y: 0,
    maxSigma: 32,
    quality: 0.5,
  },
})

const output = await sharp(blurred, {
  raw: {
    width: info.width,
    height: info.height,
    channels: info.channels,
  },
})
  .jpeg()
  .toBuffer()
```

## Debug UI

```bash
cargo run -p variable_blur_debug_ui -r
```

内置 [egui](https://github.com/emilk/egui) 桌面工具，可实时调节参数。

## API

### `variableBlur(input): Buffer`

| 参数            | 类型     | 说明                               |
| :-------------- | :------- | :--------------------------------- |
| `input.buffer`  | `Buffer` | 编码后的图片（PNG、JPEG、WebP 等） |
| `input.options` | `object` | 必填配置对象，见下方               |

### `variableBlurRaw(input): Buffer`

适合直接接 `sharp.raw().toBuffer({ resolveWithObject: true })` 的输出。

| 参数             | 类型                  | 说明                                       |
| :--------------- | :-------------------- | :----------------------------------------- |
| `input.data`     | `Buffer`              | 交错排列的原始像素字节流                   |
| `input.width`    | `number`              | 图片宽度（像素）                           |
| `input.height`   | `number`              | 图片高度（像素）                           |
| `input.channels` | `3 \| 4`              | 原始通道数；当前支持 `RGB` 或 `RGBA`       |
| `input.options`  | `VariableBlurOptions` | 必填配置对象，结构与 `variableBlur()` 相同 |

### Options

| 字段           | 类型     | 可选 | 默认值         | 说明                                                                            |
| :------------- | :------- | :--: | :------------- | :------------------------------------------------------------------------------ |
| `x`            | `number` |  否  | -              | 有限的模糊方向向量 X 分量                                                       |
| `y`            | `number` |  否  | -              | 有限的模糊方向向量 Y 分量                                                       |
| `start`        | `number` |  是  | 自动           | 有限的模糊起始投影坐标                                                          |
| `end`          | `number` |  是  | 自动           | 有限的模糊达到最大值的投影坐标                                                  |
| `quality`      | `number` |  是  | `0.5`          | `[0, 1]` 范围内的质量系数；越高会使用更多 sigma anchor 和更浅的金字塔层级       |
| `maxSigma`     | `number` |  否  | -              | 最大 sigma，控制模糊强度上限                                                    |
| `curve`        | `string` |  是  | `"power(1.6)"` | `"linear"`、`"power(γ)"`、`"cubic-bezier(x1,y1,x2,y2)"`；`γ` 必须是有限且 `> 0` |
| `outputFormat` | `string` |  是  | 与输入相同     | `"png"` / `"jpeg"` / `"jpg"` / `"webp"` / `"bmp"` / `"tiff"` / `"tga"`          |
| `advanced`     | `object` |  是  | &mdash;        | 见 [Advanced Options](#advanced-options)                                        |

### Advanced Options

<details>
<summary>底层金字塔配置（通常无需修改）</summary>

<br>

`advanced.mode: "auto"` 会根据 `quality`、`curve`、有效模糊区间、图片尺寸和 `maxSigma` 推导默认值。
如果同时传入其他 `advanced.*` 字段，它们仍会覆盖这些默认值。

| 字段                            | 类型     | 默认值   | 说明                              |
| :------------------------------ | :------- | :------- | :-------------------------------- |
| `advanced.mode`                 | `string` | `"auto"` | `"auto"` 或 `"manual"`            |
| `advanced.steps`                | `number` | 推导值   | 离散模糊级别数，必须 `>= 2`       |
| `advanced.maxLevels`            | `number` | 推导值   | 最大下采样深度，必须 `>= 1`       |
| `advanced.targetLocalSigma`     | `number` | 推导值   | 每级目标 local sigma，必须 `> 0`  |
| `advanced.minLocalSigma`        | `number` | 推导值   | 每级最小 local sigma，必须 `> 0`  |
| `advanced.maxLocalSigma`        | `number` | 推导值   | 每级最大 local sigma，必须 `> 0`  |
| `advanced.downsampleStageSigma` | `number` | `0.5`    | 2x 下采样后等效 sigma，必须 `> 0` |

</details>

## Benchmark

```bash
cargo run -p variable_blur_bench -r -- --image docs/benchmark.jpg --warmup 5 --runs 20
```

```
Machine       : Windows 11 Pro | AMD Ryzen 9 9950X3D 16-Core Processor | 16C / 32T
Image         : 2400x1300 | Jpeg | Rgb8 | 593.59 KiB
Benchmark     : 5 warmup | 20 measured
Direction     : [1.0000, 0.0000] | start 0.0000 | end 2400.0000
Max sigma     : 32.00

Quality             avg     median        p95        min        max     MPix/s
q=0.00         65.08 ms   64.67 ms   67.08 ms   62.83 ms   67.87 ms      47.94
q=0.50         91.11 ms   90.78 ms   92.58 ms   88.62 ms   93.24 ms      34.25
q=1.00        541.93 ms  541.43 ms  546.71 ms  536.32 ms  549.56 ms       5.76
```

## License

Apache-2.0
