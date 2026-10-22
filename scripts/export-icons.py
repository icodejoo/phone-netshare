"""从 assets/logo.png 提取浅色图形，导出多档尺寸的透明度蒙版，供 Rust 托盘图标嵌入。

用法：python -I scripts/export-icons.py
输出：app/icons/mask-<边长>.a（每个文件 边长*边长 字节，仅 alpha；颜色由运行时按状态着色）
"""
from pathlib import Path

from PIL import Image, ImageFilter

# 需要导出的边长，对应 100%~300% 缩放下的托盘图标尺寸；须与 app/src/icon.rs 的 MASK_SIZES 一致
SIZES = [16, 20, 24, 32, 48]
# 在 512 工作画布上提取蒙版，亮度低于 LO 的当背景，高于 HI 的当图形
WORK = 512
LO, HI = 90, 170
# 图形四周留白比例
PAD = 0.04

root = Path(__file__).resolve().parent.parent
src = Image.open(root / "assets" / "logo.png").convert("RGB").resize((WORK, WORK), Image.LANCZOS)

# 亮度 -> alpha：深蓝底透明，浅蓝图形不透明，边缘按亮度渐变保留抗锯齿
lum = src.convert("L")
mask = lum.point(lambda v: 0 if v <= LO else 255 if v >= HI else int((v - LO) * 255 / (HI - LO)))

# 裁到图形外框并补成正方形，让图形撑满画布
bbox = mask.getbbox()
side = max(bbox[2] - bbox[0], bbox[3] - bbox[1])
cx, cy = (bbox[0] + bbox[2]) // 2, (bbox[1] + bbox[3]) // 2
half = int(side * (1 + PAD * 2) / 2)
mask = mask.crop((cx - half, cy - half, cx + half, cy + half))

out = root / "app" / "icons"
out.mkdir(parents=True, exist_ok=True)
for size in SIZES:
    # 尺寸越小笔画越细，先按比例加粗再缩小，避免缩成发虚的半透明线
    k = 5 if size <= 16 else 3 if size <= 24 else 1
    m = mask.filter(ImageFilter.MaxFilter(k)) if k > 1 else mask
    data = m.resize((size, size), Image.LANCZOS).tobytes()
    (out / f"mask-{size}.a").write_bytes(data)
    print(f"mask-{size}.a ({len(data)} 字节)")
