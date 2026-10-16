@echo off
chcp 65001 >nul
setlocal

REM ============================================================================
REM release.bat - 打包 SideX 安装包（release）
REM 等价：node CUSTOMIZATIONS\scripts\release.js %*
REM 用法：
REM   双击                     -> 自动把 custom.N 递增 1 后打包
REM   release.bat 0.1.3-custom.5 -> 指定版本打包
REM 流程：版本号四处同步 -> npm run build:custom -> 列出 target\release\bundle\ 产物
REM 说明：产物在仓库根 target/，不是 src-tauri/target/（见 docs/pitfalls.md #4）
REM ============================================================================

cd /d "%~dp0..\.."
if errorlevel 1 (
    echo [X] 无法切换到仓库根目录
    pause
    exit /b 1
)

if not exist "node_modules\" (
    echo [!] 未找到 node_modules，请先运行: npm install
    pause
    exit /b 1
)

node "CUSTOMIZATIONS\scripts\release.js" %*
set "ERR=%ERRORLEVEL%"

if not "%ERR%"=="0" (
    echo.
    echo [X] release 脚本退出码 %ERR%
)

echo.
pause
exit /b %ERR%
