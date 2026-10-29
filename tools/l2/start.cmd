@echo off
rem L2 distribution launcher for Windows (fandhe-workshop-l2). Extracted next to miniserve.exe and site\ (D-58, DLV-22, TASK-198).
rem Purpose: keep the learner steps at three (extract, run start.cmd, open the URL), same as start.sh.
rem Serves site\ with miniserve on 127.0.0.1 only (DLV-14). Stored with LF in the repo; tools/build-l2.mjs converts to CRLF.
rem Kept ASCII-only because cmd reads .cmd files in the OEM code page (932 on Japanese Windows).
rem Usage: start.cmd   (set PORT=1..65535 to change the port; default 8787)
rem Exit codes: 2 = invalid PORT, 1 = cannot change to the extracted folder, otherwise miniserve's exit status.
setlocal EnableExtensions DisableDelayedExpansion
rem Change directory before enabling delayed expansion so a "!" in the folder name does not break it.
cd /d "%~dp0" || exit /b 1
setlocal EnableDelayedExpansion

rem PORT is only referenced as !PORT! and validated first, so "&", "|" or quotes in it cannot build a command.
if not defined PORT set "PORT=8787"
if "!PORT!"=="" goto badport
set "rest=!PORT!"
for %%D in (0 1 2 3 4 5 6 7 8 9) do set "rest=!rest:%%D=!"
if defined rest goto badport
if "!PORT:~0,1!"=="0" goto badport
if not "!PORT:~5!"=="" goto badport
if !PORT! GTR 65535 goto badport

set "URL=http://127.0.0.1:!PORT!/"
echo Starting. Open this URL in Chrome: !URL!
echo Press Ctrl-C to stop.

rem .\ keeps cmd from picking a same-named program from PATH. -i limits listening to 127.0.0.1.
.\miniserve.exe --index index.html -i 127.0.0.1 -p !PORT! site
exit /b !errorlevel!

:badport
echo error: PORT must be a number from 1 to 65535 1>&2
exit /b 2
