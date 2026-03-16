<div align="center">

# Variable Blur

**iOS-style variable blur for Node.js**

[![NPM Version](https://img.shields.io/npm/v/variable-blur?style=flat-square)](https://www.npmjs.com/package/variable-blur)

<img src="docs/example-blurred.png" width="500">

</div>

## Installation

```bash
npm install variable-blur
```

Prebuilt binaries for macOS / Linux / Windows (x64 & arm64).

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

`variableBlur` accepts encoded image bytes as input and returns encoded image bytes as output.

## Debug UI

```bash
cargo run -p variable_blur_debug_ui -r
```

Built-in [egui](https://github.com/emilk/egui) tool for real-time parameter tuning.

## API

### `variableBlur(input): Buffer`

| Parameter       | Type     | Description                           |
| :-------------- | :------- | :------------------------------------ |
| `input.buffer`  | `Buffer` | Encoded image (PNG, JPEG, WebP, etc.) |
| `input.options` | `object` | Required options object; see below    |

### Options

| Field          | Type     | Optional | Default        | Description                                                                           |
| :------------- | :------- | :------: | :------------- | :------------------------------------------------------------------------------------ |
| `x`            | `number` |    no    | -              | Finite X component of the blur direction vector                                       |
| `y`            | `number` |    no    | -              | Finite Y component of the blur direction vector                                       |
| `start`        | `number` |   yes    | auto           | Finite projection coordinate where blur begins                                        |
| `end`          | `number` |   yes    | auto           | Finite projection coordinate where blur reaches max                                   |
| `preset`       | `string` |   yes    | `"balanced"`   | `"fast"` / `"balanced"` / `"high"` quality preset for internal blur pyramid tuning    |
| `maxSigma`     | `number` |    no    | -              | Maximum Gaussian sigma; controls the blur strength cap                                |
| `curve`        | `string` |   yes    | `"power(1.6)"` | `"linear"`, `"power(γ)"`, `"cubic-bezier(x1,y1,x2,y2)"`; `γ` must be finite and `> 0` |
| `schedule`     | `string` |   yes    | `"power(2.8)"` | `"linear"`, `"power(γ)"`; `γ` must be finite and `> 0`                                |
| `outputFormat` | `string` |   yes    | same as input  | `"png"` / `"jpeg"` / `"jpg"` / `"webp"` / `"bmp"` / `"tiff"` / `"tga"`                |
| `advanced`     | `object` |   yes    | &mdash;        | See [Advanced Options](#advanced-options)                                             |

### Advanced Options

<details>
<summary>Low-level pyramid configuration (usually unnecessary)</summary>

<br>

`advanced.mode: "auto"` derives defaults from `preset`, image size, and `maxSigma`.
If you also provide other `advanced.*` fields, they still override those defaults.

| Field                           | Type     | Default  | Description                                         |
| :------------------------------ | :------- | :------- | :-------------------------------------------------- |
| `advanced.mode`                 | `string` | `"auto"` | `"auto"` or `"manual"`                              |
| `advanced.steps`                | `number` | derived  | Discrete blur levels, must be `>= 2`                |
| `advanced.maxLevels`            | `number` | derived  | Max downsampling depth, must be `>= 1`              |
| `advanced.targetLocalSigma`     | `number` | derived  | Per-level target local sigma, must be `> 0`         |
| `advanced.minLocalSigma`        | `number` | derived  | Per-level min local sigma, must be `> 0`            |
| `advanced.maxLocalSigma`        | `number` | derived  | Per-level max local sigma, must be `> 0`            |
| `advanced.downsampleStageSigma` | `number` | `0.5`    | Equivalent sigma after 2x downsample, must be `> 0` |

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
Sigma override: preset default

Preset              avg     median        p95        min        max     MPix/s
Fast           82.16 ms   82.13 ms   83.57 ms   80.15 ms   83.92 ms      37.98
Balanced      101.69 ms  101.68 ms  103.58 ms   97.86 ms  103.94 ms      30.68
High          150.06 ms  150.02 ms  152.08 ms  147.99 ms  153.17 ms      20.79
```

## License

Apache-2.0
