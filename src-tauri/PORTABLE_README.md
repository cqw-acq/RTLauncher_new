# RTLauncher Linux 便携版使用说明

## 安装依赖

RTLauncher 便携版需要以下系统依赖才能运行：

1. **libwebkit2gtk-4.1-dev** - WebKit 渲染引擎
2. **libappindicator3-dev** - 系统托盘支持
3. **librsvg2-dev** - SVG 图标支持
4. **patchelf** - 二进制文件处理工具

## 自动安装依赖

我们提供了自动安装脚本，支持主流 Linux 发行版：

```bash
chmod +x install-deps.sh
sudo ./install-deps.sh
```

该脚本会自动检测您的系统类型并安装相应的依赖包。

## 支持的发行版

- Ubuntu/Debian
- Fedora/RHEL/CentOS
- Arch/Manjaro/EndeavourOS
- openSUSE

## 手动安装依赖

如果自动脚本无法工作，您可以手动安装依赖：

### Ubuntu/Debian
```bash
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

### Fedora/RHEL/CentOS
```bash
sudo dnf install -y webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel patchelf
```

### Arch/Manjaro
```bash
sudo pacman -S --needed webkit2gtk-4.1 libappindicator-gtk3 librsvg patchelf
```

### openSUSE
```bash
sudo zypper install -y webkit2gtk-4.1-devel libappindicator3-devel librsvg-devel patchelf
```

## 运行 RTLauncher

安装依赖后，直接运行二进制文件：

```bash
chmod +x rtlauncher
./rtlauncher
```

## 故障排除

如果遇到缺少库的错误，请确保：

1. 已正确安装所有依赖
2. 系统包管理器已更新
3. 对于某些发行版，可能需要安装额外的开发包

如遇问题，请查看 GitHub Issues 或提交新的 issue。
