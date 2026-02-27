@echo off
REM ========================================
REM Gestalt MCP Launcher
REM ========================================

echo.
echo 🧠 Gestalt MCP Server Launcher
echo ========================================
echo.

REM Check if port 3000 is in use
netstat -ano | findstr ":3000" >nul
if %errorlevel% equ 0 (
    echo ⚠️ Port 3000 is already in use!
    echo Trying to kill existing process...
    for /f "tokens=5" %%a in ('netstat -ano ^| findstr ":3000" ^| findstr "LISTENING"') do (
        taskkill /F /PID %%a >nul 2>&1
    )
    timeout /t 2 /nobreak >nul
)

REM Start the server
echo 🚀 Starting Gestalt MCP Server...
echo 📍 Endpoint: http://127.0.0.1:3000
echo 🌐 Tools: http://127.0.0.1:3000/tools
echo 📡 MCP: http://127.0.0.1:3000/mcp
echo.

cd /d E:\scripts-python\gestalt-rust
cargo run -p gestalt_mcp -- --http

pause
