@echo off
setlocal
set "TESTROOT=%TEMP%\FileSpring-verify-%RANDOM%-%RANDOM%-%RANDOM%"
echo FileSpring: isolated generated-file verification
echo Fixture: "%TESTROOT%"
"%~dp0FileSpring-cli.exe" self-test --fixture "%TESTROOT%"
if errorlevel 1 goto failed
echo.
echo Opening the native window for 3 seconds. This does not test all GUI actions.
start "" /wait "%~dp0FileSpring.exe" --root "%TESTROOT%" --state "%TESTROOT%\gui-state" --smoke-seconds 3
set "WINDOW_RESULT=%ERRORLEVEL%"
echo Window process exit code: %WINDOW_RESULT% > "%TESTROOT%\window-smoke.txt"
if not "%WINDOW_RESULT%"=="0" goto failed
echo.
echo PASSED: generated-file checks and native process startup.
echo Report: "%TESTROOT%\verification.json"
echo Please also record Windows version using winver and test Launch-Demo.cmd.
pause
exit /b 0
:failed
echo.
echo FAILED. Keep the console error and any report under "%TESTROOT%".
pause
exit /b 1
