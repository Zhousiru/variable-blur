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
    preset: 'balanced',
  },
})

await writeFile('photo-blurred.jpg', output)
```

`variableBlur` 的输入和输出都是编码后的图片字节流。

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

### Options

| 字段           | 类型     | 可选 | 默认值         | 说明                                                                            |
| :------------- | :------- | :--: | :------------- | :------------------------------------------------------------------------------ |
| `x`            | `number` |  否  | -              | 有限的模糊方向向量 X 分量                                                       |
| `y`            | `number` |  否  | -              | 有限的模糊方向向量 Y 分量                                                       |
| `start`        | `number` |  是  | 自动           | 有限的模糊起始投影坐标                                                          |
| `end`          | `number` |  是  | 自动           | 有限的模糊达到最大值的投影坐标                                                  |
| `preset`       | `string` |  是  | `"balanced"`   | `"fast"` / `"balanced"` / `"high"`，用于内部质量档位和高级参数                  |
| `maxSigma`     | `number` |  否  | -              | 最大 sigma，控制模糊强度上限                                                    |
| `curve`        | `string` |  是  | `"power(1.6)"` | `"linear"`、`"power(γ)"`、`"cubic-bezier(x1,y1,x2,y2)"`；`γ` 必须是有限且 `> 0` |
| `schedule`     | `string` |  是  | `"power(2.8)"` | `"linear"`、`"power(γ)"`；`γ` 必须是有限且 `> 0`                                |
| `outputFormat` | `string` |  是  | 与输入相同     | `"png"` / `"jpeg"` / `"jpg"` / `"webp"` / `"bmp"` / `"tiff"` / `"tga"`          |
| `advanced`     | `object` |  是  | &mdash;        | 见 [Advanced Options](#advanced-options)                                        |

### Advanced Options

<details>
<summary>底层金字塔配置（通常无需修改）</summary>

<br>

`advanced.mode: "auto"` 会根据 `preset`、图片尺寸和 `maxSigma` 推导默认值。
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
Machine       : macOS 26.2 | Apple M3 Pro | 12C / 12T
Image         : 2400x1300 | Jpeg | Rgb8 | 593.59 KiB
Benchmark     : 5 warmup | 20 measured
Direction     : [1.0000, 0.0000] | start 0.0000 | end 2400.0000
Sigma override: preset default

Preset              avg     median        p95        min        max     MPix/s
Fast           34.81 ms   33.90 ms   39.63 ms   32.71 ms   43.96 ms      89.62
Balanced       41.43 ms   40.06 ms   49.44 ms   39.29 ms   52.31 ms      75.30
High           62.48 ms   59.09 ms   88.07 ms   57.37 ms   88.39 ms      49.94


Machine       : Windows 11 Pro | AMD Ryzen 9 9950X3D 16-Core Processor | 16C / 32T
Image         : 2400x1300 | Jpeg | Rgb8 | 593.59 KiB
Benchmark     : 5 warmup | 20 measured
Direction     : [1.0000, 0.0000] | start 0.0000 | end 2400.0000
Sigma override: preset default

Preset              avg     median        p95        min        max     MPix/s
Fast           82.16 ms   82.13 ms   83.57 ms   80.15 ms   83.92 ms      37.98
Balanced      101.69 ms  101.68 ms  103.58 ms   97.86 ms  103.94 ms      30.68
High          150.06 ms  150.02 ms  152.08 ms  147.99 ms  153.17 ms      20.79
```

## License

Apache-2.0
