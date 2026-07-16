"use client";

import { useEffect, useRef, useState } from "react";

/**
 * 3D Minecraft 皮肤查看器（基于 Canvas 2D，无外部依赖）
 * - 使用正交投影渲染站立姿势的史蒂夫
 * - 支持鼠标/触摸拖拽旋转
 * - 自动轻微摆动动画
 */

type SkinViewer3DProps = {
  skinSrc: string; // base64 PNG 或 URL
  playerName?: string;
  width?: number;
  height?: number;
};

// Minecraft 皮肤贴图分区（64x32 / 64x64 通用）
const TEX_PARTS = {
  HEAD_FRONT: { x: 8, y: 8, w: 8, h: 8 },
  HEAD_BACK: { x: 24, y: 8, w: 8, h: 8 },
  HEAD_LEFT: { x: 0, y: 8, w: 8, h: 8 },
  HEAD_RIGHT: { x: 16, y: 8, w: 8, h: 8 },
  HEAD_TOP: { x: 8, y: 0, w: 8, h: 8 },
  HEAD_BOTTOM: { x: 16, y: 0, w: 8, h: 8 },
  BODY_FRONT: { x: 20, y: 20, w: 8, h: 12 },
  BODY_BACK: { x: 32, y: 20, w: 8, h: 12 },
  BODY_LEFT: { x: 16, y: 20, w: 4, h: 12 },
  BODY_RIGHT: { x: 28, y: 20, w: 4, h: 12 },
  BODY_TOP: { x: 20, y: 16, w: 8, h: 4 },
  BODY_BOTTOM: { x: 28, y: 16, w: 8, h: 4 },
  ARM_R_FRONT: { x: 44, y: 20, w: 4, h: 12 },
  ARM_R_BACK: { x: 52, y: 20, w: 4, h: 12 },
  ARM_R_LEFT: { x: 40, y: 20, w: 4, h: 12 },
  ARM_R_RIGHT: { x: 48, y: 20, w: 4, h: 12 },
  ARM_R_TOP: { x: 44, y: 16, w: 4, h: 4 },
  ARM_R_BOTTOM: { x: 48, y: 16, w: 4, h: 4 },
  LEG_R_FRONT: { x: 4, y: 20, w: 4, h: 12 },
  LEG_R_BACK: { x: 12, y: 20, w: 4, h: 12 },
  LEG_R_LEFT: { x: 0, y: 20, w: 4, h: 12 },
  LEG_R_RIGHT: { x: 8, y: 20, w: 4, h: 12 },
  LEG_R_TOP: { x: 4, y: 16, w: 4, h: 4 },
  LEG_R_BOTTOM: { x: 8, y: 16, w: 4, h: 4 },
};

// 3D 立方体定义：中心位置 + 尺寸
type Box3D = {
  cx: number; // 世界坐标 X 中心
  cy: number; // 世界坐标 Y 中心（向下为正）
  cz: number; // 世界坐标 Z 中心
  w: number;
  h: number;
  d: number;
  tex: {
    front: { x: number; y: number; w: number; h: number };
    back: { x: number; y: number; w: number; h: number };
    left: { x: number; y: number; w: number; h: number };
    right: { x: number; y: number; w: number; h: number };
    top: { x: number; y: number; w: number; h: number };
    bottom: { x: number; y: number; w: number; h: number };
  };
  mirrorX?: boolean; // 用于左臂/左腿（从右臂/腿镜像）
  rotOffset?: { x: number; y: number; z: number }; // 旋转偏移（度数）
};

function makeBoxes(): Box3D[] {
  // 每个像素=1 单位。原点在脚底中心，Y 轴向下（屏幕）向上（世界）
  // 但为了简单：我们让 Y 向下为正，相机从上方看
  return [
    // 头（8x8x8，中心在身体中心上方，Y 向下，所以头的 cy 较小）
    {
      cx: 0,
      cy: -8,
      cz: 0,
      w: 8,
      h: 8,
      d: 8,
      tex: {
        front: TEX_PARTS.HEAD_FRONT,
        back: TEX_PARTS.HEAD_BACK,
        left: TEX_PARTS.HEAD_LEFT,
        right: TEX_PARTS.HEAD_RIGHT,
        top: TEX_PARTS.HEAD_TOP,
        bottom: TEX_PARTS.HEAD_BOTTOM,
      },
    },
    // 身体（8x12x4）
    {
      cx: 0,
      cy: 2, // 头下方：头 cy=-8, h=8，所以头底 cy=0, 身体从 0 开始
      cz: 0,
      w: 8,
      h: 12,
      d: 4,
      tex: {
        front: TEX_PARTS.BODY_FRONT,
        back: TEX_PARTS.BODY_BACK,
        left: TEX_PARTS.BODY_LEFT,
        right: TEX_PARTS.BODY_RIGHT,
        top: TEX_PARTS.BODY_TOP,
        bottom: TEX_PARTS.BODY_BOTTOM,
      },
    },
    // 右臂（4x12x4，身体右侧）
    {
      cx: -6,
      cy: 2,
      cz: 0,
      w: 4,
      h: 12,
      d: 4,
      tex: {
        front: TEX_PARTS.ARM_R_FRONT,
        back: TEX_PARTS.ARM_R_BACK,
        left: TEX_PARTS.ARM_R_LEFT,
        right: TEX_PARTS.ARM_R_RIGHT,
        top: TEX_PARTS.ARM_R_TOP,
        bottom: TEX_PARTS.ARM_R_BOTTOM,
      },
    },
    // 左臂（镜像右臂）
    {
      cx: 6,
      cy: 2,
      cz: 0,
      w: 4,
      h: 12,
      d: 4,
      tex: {
        front: TEX_PARTS.ARM_R_FRONT,
        back: TEX_PARTS.ARM_R_BACK,
        left: TEX_PARTS.ARM_R_LEFT,
        right: TEX_PARTS.ARM_R_RIGHT,
        top: TEX_PARTS.ARM_R_TOP,
        bottom: TEX_PARTS.ARM_R_BOTTOM,
      },
      mirrorX: true,
    },
    // 右腿（4x12x4）
    {
      cx: -2,
      cy: 14,
      cz: 0,
      w: 4,
      h: 12,
      d: 4,
      tex: {
        front: TEX_PARTS.LEG_R_FRONT,
        back: TEX_PARTS.LEG_R_BACK,
        left: TEX_PARTS.LEG_R_LEFT,
        right: TEX_PARTS.LEG_R_RIGHT,
        top: TEX_PARTS.LEG_R_TOP,
        bottom: TEX_PARTS.LEG_R_BOTTOM,
      },
    },
    // 左腿（镜像右腿）
    {
      cx: 2,
      cy: 14,
      cz: 0,
      w: 4,
      h: 12,
      d: 4,
      tex: {
        front: TEX_PARTS.LEG_R_FRONT,
        back: TEX_PARTS.LEG_R_BACK,
        left: TEX_PARTS.LEG_R_LEFT,
        right: TEX_PARTS.LEG_R_RIGHT,
        top: TEX_PARTS.LEG_R_TOP,
        bottom: TEX_PARTS.LEG_R_BOTTOM,
      },
      mirrorX: true,
    },
  ];
}

export function SkinViewer3D({ skinSrc, width = 320, height = 400 }: SkinViewer3DProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const skinImgRef = useRef<HTMLImageElement | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState(false);

  // 旋转状态
  const yawRef = useRef(25 * (Math.PI / 180)); // 水平旋转（弧度）
  const pitchRef = useRef(10 * (Math.PI / 180)); // 俯仰角
  const dragStateRef = useRef<{ dragging: boolean; lastX: number; lastY: number }>({
    dragging: false,
    lastX: 0,
    lastY: 0,
  });
  const animRef = useRef<number | null>(null);
  const timeStartRef = useRef<number>(performance.now());

  // 加载皮肤贴图
  useEffect(() => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      skinImgRef.current = img;
      setLoaded(true);
      setError(false);
    };
    img.onerror = () => {
      setError(true);
    };
    img.src = skinSrc;
  }, [skinSrc]);

  // 渲染循环
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const render = () => {
      // 清屏
      ctx.clearRect(0, 0, canvas.width, canvas.height);

      // 背景（轻微渐变）
      const gradient = ctx.createLinearGradient(0, 0, 0, canvas.height);
      gradient.addColorStop(0, "rgba(0,0,0,0.02)");
      gradient.addColorStop(1, "rgba(0,0,0,0.06)");
      ctx.fillStyle = gradient;
      ctx.fillRect(0, 0, canvas.width, canvas.height);

      if (!loaded || !skinImgRef.current) {
        animRef.current = requestAnimationFrame(render);
        return;
      }

      const skinImg = skinImgRef.current;

      // 自动轻微旋转 + 上下摆动
      const t = (performance.now() - timeStartRef.current) / 1000;
      let yaw = yawRef.current;
      let pitch = pitchRef.current;

      if (!dragStateRef.current.dragging) {
        yaw += 0.3 * (Math.PI / 180); // 每秒 0.3 度
      }

      const bobY = Math.sin(t * 2) * 0.5; // 轻微上下摆动

      // 相机参数
      const scale = Math.min(canvas.width, canvas.height) / 30; // 每单位像素
      const centerX = canvas.width / 2;
      const centerY = canvas.height / 2 + 4 * scale; // 向下偏移让角色居中

      // 旋转矩阵（简化：先绕 Y 轴旋转 yaw，再绕 X 轴旋转 pitch）
      const sinY = Math.sin(yaw);
      const cosY = Math.cos(yaw);
      const sinP = Math.sin(pitch);
      const cosP = Math.cos(pitch);

      // 投影函数：3D 世界坐标 → 2D 屏幕坐标，同时返回近似深度
      const project = (x: number, y: number, z: number): { sx: number; sy: number; depth: number } => {
        // y 轴在世界中向下为正，我们这里用屏幕坐标（向下为正）
        // 先绕 Y 轴旋转 (x, z)
        const x1 = x * cosY - z * sinY;
        const z1 = x * sinY + z * cosY;
        // 再绕 X 轴旋转 (y, z1) —— 但注意 y 轴方向：向下为正
        const y2 = y * cosP - z1 * sinP;
        const z2 = y * sinP + z1 * cosP;

        // 正交投影：z2 越大越深（越远）
        const sx = centerX + x1 * scale;
        const sy = centerY + y2 * scale;
        return { sx, sy, depth: z2 };
      };

      // 收集所有可见面并按深度排序（画家算法）
      type Face = {
        pts: { sx: number; sy: number }[];
        avgDepth: number;
        texture: { x: number; y: number; w: number; h: number };
        shade: number;
        mirror: boolean;
      };

      const faces: Face[] = [];

      const boxes = makeBoxes();

      for (const box of boxes) {
        // 计算 8 个顶点（在世界坐标，Y 向下为正，但我们需要 Y 向上：让 Y 轴反一下）
        // 但为了简单，这里直接计算立方体的 min/max 坐标：
        const hw = box.w / 2;
        const hh = box.h / 2;
        const hd = box.d / 2;

        // 将 Y 轴反转（让 cy 向上为正，投影前再转回来）
        // 简化：直接用 y = box.cy，但为了让头在上，让所有 cy 取负（向上为正）
        // 这是最重要的：Minecraft Y 轴向下为正，我们希望头在上，所以投影时用 -y
        const cy = -box.cy;

        const minX = box.cx - hw;
        const maxX = box.cx + hw;
        const minY = cy - hh; // 向上为正，minY 更低（脚底方向）
        const maxY = cy + hh;
        const minZ = box.cz - hd;
        const maxZ = box.cz + hd;

        // 8 个顶点（注意 Y 轴现在向上为正：maxY 顶，minY 底）
        // 名字：F=前 (z 较小=minZ)，B=后(z=maxZ)，L=左(x=minX), R=右(x=maxX), T=顶(y=maxY), D=底(y=minY)
        const P = {
          FLT: project(minX, -maxY, minZ),
          FRT: project(maxX, -maxY, minZ),
          FLD: project(minX, -minY, minZ),
          FRD: project(maxX, -minY, minZ),
          BLT: project(minX, -maxY, maxZ),
          BRT: project(maxX, -maxY, maxZ),
          BLD: project(minX, -minY, maxZ),
          BRD: project(maxX, -minY, maxZ),
        };

        // 添加一个 bobY（整个角色上下轻微摆动）
        const addBob = (p: { sx: number; sy: number; depth: number }) => ({
          sx: p.sx,
          sy: p.sy + bobY * scale,
          depth: p.depth,
        });

        const pL = addBob(P.FLT); const pR = addBob(P.FRT); const pLD = addBob(P.FLD); const pRD = addBob(P.FRD);
        const pBLT = addBob(P.BLT); const pBRT = addBob(P.BRT); const pBLD = addBob(P.BLD); const pBRD = addBob(P.BRD);

        // 前面（z = minZ，朝向相机）—— front 贴图
        faces.push({
          pts: [pL, pR, pRD, pLD],
          avgDepth: (P.FLT.depth + P.FRT.depth + P.FLD.depth + P.FRD.depth) / 4,
          texture: box.tex.front,
          shade: 1.0,
          mirror: !!box.mirrorX,
        });
        // 后面（z = maxZ）
        faces.push({
          pts: [pBRT, pBLT, pBLD, pBRD],
          avgDepth: (P.BLT.depth + P.BRT.depth + P.BLD.depth + P.BRD.depth) / 4,
          texture: box.tex.back,
          shade: 0.7,
          mirror: !!box.mirrorX,
        });
        // 左面（x = minX）——注意镜像
        faces.push({
          pts: [pBLT, pL, pLD, pBLD],
          avgDepth: (P.BLT.depth + P.FLT.depth + P.FLD.depth + P.BLD.depth) / 4,
          texture: box.mirrorX ? box.tex.right : box.tex.left,
          shade: 0.85,
          mirror: false,
        });
        // 右面（x = maxX）
        faces.push({
          pts: [pR, pBRT, pBRD, pRD],
          avgDepth: (P.FRT.depth + P.BRT.depth + P.BRD.depth + P.FRD.depth) / 4,
          texture: box.mirrorX ? box.tex.left : box.tex.right,
          shade: 0.85,
          mirror: false,
        });
        // 顶面（y = maxY）
        faces.push({
          pts: [pL, pBLT, pBRT, pR],
          avgDepth: (P.FLT.depth + P.BLT.depth + P.BRT.depth + P.FRT.depth) / 4,
          texture: box.tex.top,
          shade: 1.1,
          mirror: !!box.mirrorX,
        });
        // 底面（y = minY）
        faces.push({
          pts: [pLD, pRD, pBRD, pBLD],
          avgDepth: (P.FLD.depth + P.FRD.depth + P.BRD.depth + P.BLD.depth) / 4,
          texture: box.tex.bottom,
          shade: 0.55,
          mirror: !!box.mirrorX,
        });
      }

      // 按深度排序（从远到近）
      faces.sort((a, b) => b.avgDepth - a.avgDepth);

      // 绘制每个面
      for (const face of faces) {
        // 从皮肤贴图裁剪
        const sourceX = face.texture.x;
        const sourceY = face.texture.y;
        const sourceW = face.texture.w;
        const sourceH = face.texture.h;

        // 计算屏幕多边形的包围盒
        const minSx = Math.min(...face.pts.map(p => p.sx));
        const maxSx = Math.max(...face.pts.map(p => p.sx));
        const minSy = Math.min(...face.pts.map(p => p.sy));
        const maxSy = Math.max(...face.pts.map(p => p.sy));

        // 离屏 canvas 处理贴图绘制
        const off = document.createElement("canvas");
        off.width = Math.max(2, Math.ceil(maxSx - minSx));
        off.height = Math.max(2, Math.ceil(maxSy - minSy));
        const offCtx = off.getContext("2d");
        if (!offCtx) continue;

        // 将源贴图裁剪并缩放到离屏 canvas
        offCtx.save();
        if (face.mirror) {
          offCtx.translate(off.width, 0);
          offCtx.scale(-1, 1);
        }
        offCtx.imageSmoothingEnabled = false;
        offCtx.drawImage(
          skinImg,
          sourceX, sourceY, sourceW, sourceH,
          0, 0, off.width, off.height
        );
        offCtx.restore();

        // 阴影处理：改变亮度（通过 globalCompositeOperation + 半透明覆盖）
        if (face.shade !== 1.0) {
          offCtx.globalCompositeOperation = "source-atop";
          const gray = Math.max(0, Math.min(255, Math.round(face.shade > 1.0 ? 255 : 255 * face.shade)));
          // 更简单：如果 shade<1 则叠黑色，如果 >1 叠白色
          if (face.shade < 1.0) {
            offCtx.fillStyle = `rgba(0,0,0,${1.0 - face.shade})`;
          } else {
            offCtx.fillStyle = `rgba(255,255,255,${Math.min(1, face.shade - 1.0)})`;
          }
          offCtx.fillRect(0, 0, off.width, off.height);
          offCtx.globalCompositeOperation = "source-over";
        }

        // 将离屏 canvas 作为图案绘制到目标四边形
        // 使用 clip + transform 的方式：
        ctx.save();
        // 构建多边形路径
        ctx.beginPath();
        ctx.moveTo(face.pts[0].sx, face.pts[0].sy);
        for (let i = 1; i < face.pts.length; i++) {
          ctx.lineTo(face.pts[i].sx, face.pts[i].sy);
        }
        ctx.closePath();
        ctx.clip();

        // 仿射变换：离屏 canvas 左上角（0,0） -> (minSx, minSy)，宽高 -> off.width, off.height
        // 直接平铺贴图（已经裁剪好的）：但对于非矩形（透视投影）需要更复杂的处理
        // 简化：直接 drawImage 矩形，配合 clip，虽然不精确但足够展示
        ctx.drawImage(off, minSx, minSy);

        // 描边：绘制面边缘（1px 深色线，让轮廓更清晰）
        ctx.restore();

        ctx.save();
        ctx.strokeStyle = "rgba(0,0,0,0.25)";
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(face.pts[0].sx, face.pts[0].sy);
        for (let i = 1; i < face.pts.length; i++) {
          ctx.lineTo(face.pts[i].sx, face.pts[i].sy);
        }
        ctx.closePath();
        ctx.stroke();
        ctx.restore();
      }

      animRef.current = requestAnimationFrame(render);
    };

    animRef.current = requestAnimationFrame(render);
    return () => {
      if (animRef.current !== null) cancelAnimationFrame(animRef.current);
    };
  }, [loaded]);

  // 鼠标/触摸交互
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const handleMouseDown = (e: MouseEvent) => {
      dragStateRef.current = { dragging: true, lastX: e.clientX, lastY: e.clientY };
    };
    const handleMouseMove = (e: MouseEvent) => {
      if (!dragStateRef.current.dragging) return;
      const dx = e.clientX - dragStateRef.current.lastX;
      const dy = e.clientY - dragStateRef.current.lastY;
      dragStateRef.current.lastX = e.clientX;
      dragStateRef.current.lastY = e.clientY;
      yawRef.current += dx * 0.008;
      pitchRef.current = Math.max(
        -Math.PI / 3,
        Math.min(Math.PI / 3, pitchRef.current + dy * 0.008)
      );
    };
    const handleMouseUp = () => {
      dragStateRef.current.dragging = false;
    };

    const handleTouchStart = (e: TouchEvent) => {
      if (e.touches.length === 1) {
        dragStateRef.current = {
          dragging: true,
          lastX: e.touches[0].clientX,
          lastY: e.touches[0].clientY,
        };
      }
    };
    const handleTouchMove = (e: TouchEvent) => {
      if (!dragStateRef.current.dragging || e.touches.length !== 1) return;
      e.preventDefault();
      const dx = e.touches[0].clientX - dragStateRef.current.lastX;
      const dy = e.touches[0].clientY - dragStateRef.current.lastY;
      dragStateRef.current.lastX = e.touches[0].clientX;
      dragStateRef.current.lastY = e.touches[0].clientY;
      yawRef.current += dx * 0.008;
      pitchRef.current = Math.max(
        -Math.PI / 3,
        Math.min(Math.PI / 3, pitchRef.current + dy * 0.008)
      );
    };
    const handleTouchEnd = () => {
      dragStateRef.current.dragging = false;
    };

    canvas.addEventListener("mousedown", handleMouseDown);
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    canvas.addEventListener("touchstart", handleTouchStart, { passive: true });
    canvas.addEventListener("touchmove", handleTouchMove, { passive: false });
    canvas.addEventListener("touchend", handleTouchEnd);

    return () => {
      canvas.removeEventListener("mousedown", handleMouseDown);
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      canvas.removeEventListener("touchstart", handleTouchStart);
      canvas.removeEventListener("touchmove", handleTouchMove);
      canvas.removeEventListener("touchend", handleTouchEnd);
    };
  }, []);

  return (
    <div className="relative flex items-center justify-center select-none">
      <canvas
        ref={canvasRef}
        width={width}
        height={height}
        className="cursor-grab active:cursor-grabbing rounded-lg"
      />
      {!loaded && !error && (
        <div className="absolute inset-0 flex items-center justify-center text-sm text-muted-foreground">
          加载皮肤中...
        </div>
      )}
      {error && (
        <div className="absolute inset-0 flex items-center justify-center text-sm text-muted-foreground">
          皮肤加载失败
        </div>
      )}
    </div>
  );
}