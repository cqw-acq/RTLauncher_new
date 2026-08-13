#!/bin/bash
# RTLauncher Linux 依赖安装脚本
# 此脚本用于安装 RTLauncher 便携版所需的系统依赖

set -e

echo "正在检查系统类型..."
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS=$ID
    VERSION=$VERSION_ID
else
    echo "错误: 无法检测系统类型"
    exit 1
fi

echo "检测到系统: $OS $VERSION"

# 检查是否为 root 用户或有 sudo 权限
if [ "$EUID" -ne 0 ]; then
    if command -v sudo &> /dev/null; then
        SUDO="sudo"
    else
        echo "错误: 需要 root 权限或 sudo 来安装依赖"
        exit 1
    fi
else
    SUDO=""
fi

case $OS in
    ubuntu|debian)
        echo "正在为 Ubuntu/Debian 系统安装依赖..."
        $SUDO apt-get update
        $SUDO apt-get install -y \
            libwebkit2gtk-4.1-dev \
            libappindicator3-dev \
            librsvg2-dev \
            patchelf
        ;;
    
    fedora|rhel|centos)
        echo "正在为 Fedora/RHEL/CentOS 系统安装依赖..."
        $SUDO dnf install -y \
            webkit2gtk4.1-devel \
            libappindicator-gtk3-devel \
            librsvg2-devel \
            patchelf
        ;;
    
    arch|manjaro|endeavouros)
        echo "正在为 Arch/Manjaro 系统安装依赖..."
        $SUDO pacman -S --needed \
            webkit2gtk-4.1 \
            libappindicator-gtk3 \
            librsvg \
            patchelf
        ;;
    
    opensuse*)
        echo "正在为 openSUSE 系统安装依赖..."
        $SUDO zypper install -y \
            webkit2gtk-4.1-devel \
            libappindicator3-devel \
            librsvg-devel \
            patchelf
        ;;
    
    *)
        echo "警告: 未知的系统类型 $OS"
        echo "请手动安装以下依赖:"
        echo "  - webkit2gtk-4.1 (或对应的 webkit2gtk 开发包)"
        echo "  - libappindicator3 (或对应的开发包)"
        echo "  - librsvg2 (或对应的开发包)"
        echo "  - patchelf"
        exit 1
        ;;
esac

echo "✅ 依赖安装完成！"
echo "现在可以运行 RTLauncher 了"
