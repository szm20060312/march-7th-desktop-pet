# 三月七图集局部修正（2026-10-03）

修正应用使用的 `march-7th-app/public/assets/march-7th/spritesheet.webp`。
行、帧均从 1 开始计数；运行时行列索引从 0 开始。

| 修正帧 | 图集位置 | 修正内容 |
| --- | --- | --- |
| `waving-02.png` | 第 4 行，第 2 帧 | 去掉抬手一侧仍握着相机的第三只手和旧前臂 |
| `waving-03.png` | 第 4 行，第 3 帧 | 去掉另一侧挥手时遗留的握相机的手及旧袖口 |
| `running-04.png` | 第 8 行，第 4 帧 | 将相机右侧两个重叠袖口合为一个完整袖子 |
| `running-06.png` | 第 8 行，第 6 帧 | 去掉举拳一侧的第三只手，并修正另一侧重复袖口 |

## 文件和验证

- 四张 PNG 是最终合成后的 192 × 208 透明帧，可直接替换对应格子。
- `comparison.png` 上排为原图，下排为修正图。
- `waving-preview.gif`、`camera-preview.gif` 为四帧挥手和六帧相机动作的循环预览。播放间隔仅用于视觉检查。
- `changes.json` 记录局部替换多边形，其坐标相对于每个 192 × 208 格子。
- `verification.json` 记录源文件和修正图集的 SHA-256，以及逐像素检查结果。

使用内置 `imagegen` 编辑四张放大到 768 × 832 的源帧。将生成结果缩回格子尺寸后，只提取修复部位，合回原帧；没有用整张生成图替换原帧。图集以精确无损 WebP 保存，保留透明像素的原始 RGB。

最终图集仍为 1536 × 2288（8 列、11 行），四个指定格子以外的 84 个格子 RGBA 逐像素相同，完全透明像素的 RGB 残留为零。此次检查覆盖肢体结构、局部接缝和相邻帧姿态；不代表应用已接入这两个动作。当前应用角色配置只启用待机、左右移动与方向注视。

`releases/v1.0.0/` 是带校验清单的历史发布包，保持归档原样；此修正用于独立应用，旧版发布包的校验结果不作为修正图集的验证记录。

## 使用的提示词

前三次编辑使用以下公共约束，再追加对应帧的指令：

```text
Use case: precise-object-edit. Asset type: existing game sprite, local anatomy repair. Edit target is the sole input image. Output a transparent PNG with the exact same 768x832 canvas, exact same character size, location, silhouette and pixelated source texture. This is a tiny inpainting repair, not a redraw. Keep the face, hair, expression, raised hand/arm, outfit, bow, legs, shoes and ribbon completely unchanged. Preserve all existing pixel colors and outlines outside the small defective hand/sleeve area. No text, no new elements. Exactly two hands and two arms after repair, one raised hand and the opposite hand holding the camera. The camera must stay in its original position, size and angle.
```

`waving-02`：

```text
The hand on the IMAGE LEFT gripping the left side of the camera is a THIRD, impossible hand; it and its small lowered forearm/sleeve must be removed. Preserve the open waving hand and arm at image upper left. Preserve the hand and large white sleeve holding the camera on IMAGE RIGHT. Reconstruct only the background/clothing previously obscured by the extra lowered left hand and sleeve; the camera is supported by the right hand only.
```

`waving-03`：

```text
The hand on the IMAGE RIGHT gripping the right edge of the camera, under the raised waving arm, is a THIRD, impossible hand; remove this small hand and its redundant lowered cuff/arm. Preserve the open waving hand and raised white sleeve at upper right. Preserve the left hand and sleeve supporting the camera on IMAGE LEFT. Reconstruct the tiny exposed clothing/space at the camera right edge. The camera is held with the left hand only.
```

`running-06`：

```text
Remove the THIRD hand at IMAGE LEFT edge of the camera, below the raised clenched fist; the raised fist at image upper left and its white sleeve must remain. Preserve the hand holding the camera on IMAGE RIGHT. Also repair the duplicated sleeve cuff on image right: the camera-holding right hand must connect to ONE coherent white sleeve and ONE gold cuff ornament, remove the ghost second sleeve/cuff behind it. Only edit these small hand and cuff areas.
```

`running-04` 使用以下独立提示词：

```text
Use case: precise-object-edit. Edit the sole input game sprite with a tiny local anatomy repair. The camera-holding arm on IMAGE RIGHT has TWO gold wrist cuff buttons and two overlapping cuffs, revealing a ghost second sleeve behind the real one. Remove the rear duplicate cuff and button and reconstruct ONE coherent white forearm sleeve connected to the existing camera-holding hand, with ONE gold cuff ornament. Keep exactly two arms and two hands. Preserve the hand touching her mouth on image left, camera angle and position, face, hair, expression, torso, skirt, legs, shoes and ribbon entirely. Keep the same canvas aspect ratio, subject size and position. Transparent background, preserve alpha. No new elements, no text, no redesign. Change only the right sleeve immediately below/right of the camera; preserve existing source pixel texture.
```
