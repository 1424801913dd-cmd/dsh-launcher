Option Explicit

Dim shell, http, nodeExe, dshEntry, workspace, webUrl, command, quote, ready, attempt

nodeExe = "D:\Tools\node-v24.19.0-win-x64\node.exe"
dshEntry = "D:\Tools\dsh-runtime-0.1.1-rc.2\node_modules\@deepseek-ai\dsh\lib\bin.js"
workspace = "D:\Bian_CHENG\dsmax"
webUrl = "http://127.0.0.1:3080/"

Set shell = CreateObject("WScript.Shell")
shell.Environment("PROCESS")("DSH_HOME") = "D:\Caches\deepseek-harness\home"
shell.CurrentDirectory = workspace
quote = Chr(34)

Function HarnessReady()
    On Error Resume Next
    Set http = CreateObject("MSXML2.XMLHTTP.6.0")
    http.Open "GET", webUrl, False
    http.Send
    HarnessReady = (Err.Number = 0 And http.Status = 200)
    Err.Clear
    On Error GoTo 0
End Function

ready = HarnessReady()

If Not ready Then
    command = quote & nodeExe & quote & " " & quote & dshEntry & quote & _
        " web --host 127.0.0.1 --port 3080 --no-open"
    shell.Run command, 0, False

    For attempt = 1 To 180
        WScript.Sleep 500
        If HarnessReady() Then
            ready = True
            Exit For
        End If
    Next
End If

If ready Then
    shell.Run webUrl, 1, False
Else
    MsgBox "DeepSeek Harness failed to start. Check D:\Caches\deepseek-harness logs.", _
        vbCritical, "DeepSeek Harness"
End If
