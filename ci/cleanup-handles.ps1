# Read-only, exact-profile handle lookup. Never close handles or dump environments.
param([Parameter(Mandatory)][string]$Profile, [Parameter(Mandatory)][string]$OutputPath)
$ErrorActionPreference = 'Continue'
"UTC: $([DateTime]::UtcNow.ToString('o'))`nProfile: $Profile" | Out-File $OutputPath
$processes = Get-CimInstance Win32_Process -Filter "Name = 'chrome.exe' OR Name = 'chromedriver.exe' OR Name = 'chrome_crashpad_handler.exe'"
$processes | Select-Object Name, ProcessId, ParentProcessId, CreationDate,
    @{Name='MatchesProfile';Expression={$_.CommandLine -and $_.CommandLine.Contains($Profile)}} |
    ConvertTo-Json -Depth 3 | Out-File $OutputPath -Append
if ($env:THIRTYFOUR_HANDLE_EXE) {
    & $env:THIRTYFOUR_HANDLE_EXE -accepteula -nobanner $Profile 2>&1 | Out-File $OutputPath -Append
    "Handle exit code: $LASTEXITCODE" | Out-File $OutputPath -Append
} else { 'Handle executable unavailable; no claim about handle ownership.' | Out-File $OutputPath -Append }
