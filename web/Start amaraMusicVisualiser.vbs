Option Explicit
Dim shell, fso, folder, launcher, command
Set shell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")
folder = fso.GetParentFolderName(WScript.ScriptFullName)
launcher = fso.BuildPath(folder, "windows-launcher.mjs")
command = "node.exe """ & launcher & """"
On Error Resume Next
shell.Run command, 0, False
If Err.Number <> 0 Then
  MsgBox "amaraMusicVisualiser could not start. Install Node.js, then try again.", _
    vbExclamation, "amaraMusicVisualiser"
End If
On Error GoTo 0

' The Node host and its PowerShell/C# capture helper both run hidden. Audio is
' bound to loopback, stays in memory, and is never saved. If startup succeeds,
' the default browser opens the local visualiser. The host lasts for this
' Windows sign-in session and can be ended in Task Manager (node.exe).
