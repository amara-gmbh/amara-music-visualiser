$ErrorActionPreference = 'Stop'
$source = Join-Path $PSScriptRoot 'AmaraWindowsAudioCapture.cs'
try {
    Add-Type -Path $source
    exit [AmaraMusicVisualiser.WindowsAudioCapture]::Run()
} catch {
    [Console]::Error.WriteLine('Windows audio capture could not start: ' + $_.Exception.Message)
    exit 1
}
