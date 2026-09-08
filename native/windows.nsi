Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"
!ifndef STAGE_DIR
!error "Pass /DSTAGE_DIR=<verified native staging directory>"
!endif
!ifndef OUTPUT_FILE
!error "Pass /DOUTPUT_FILE=<installer path>"
!endif
!ifndef APP_VERSION
!error "Pass /DAPP_VERSION=<workspace version>"
!endif
!ifndef UNINSTALL_INCLUDE
!error "Pass /DUNINSTALL_INCLUDE=<generated package file list>"
!endif
!ifndef PRODUCT_NAME
!error "Pass /DPRODUCT_NAME, /DAPP_EXE, /DREGISTRY_KEY and /DNUMERIC_VERSION from xtask"
!endif
VIProductVersion "${NUMERIC_VERSION}"
VIAddVersionKey /LANG=1033 "ProductName" "${PRODUCT_NAME}"
VIAddVersionKey /LANG=1033 "FileDescription" "${PRODUCT_NAME} installer"
VIAddVersionKey /LANG=1033 "FileVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" ""
Name "${PRODUCT_NAME}"
OutFile "${OUTPUT_FILE}"
InstallDir "$PROGRAMFILES64\${PRODUCT_NAME}"
InstallDirRegKey HKLM "Software\${REGISTRY_KEY}" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_FUNCTION RestartRotor
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

Var RestartArguments
Var ParentPid
Var StagedDirectory
Var PreviousDirectory
Var PreviousVersion
Var InstallParent
Var FailedDirectory
Var InstallLogFile
Var InstallLogPath

; Optional /LOG is for unattended diagnostics; no log is created by default.
!macro ReportMessage FLAGS MESSAGE
  ${If} $InstallLogFile != ""
    FileWriteUTF16LE $InstallLogFile "${MESSAGE}$\r$\n"
  ${EndIf}
  MessageBox ${FLAGS} "${MESSAGE}" /SD IDOK
!macroend

Function RestartRotor
  ${If} $PreviousDirectory != ""
  ${AndIf} ${FileExists} "$PreviousDirectory\${APP_EXE}"
    ExecShell "open" "$INSTDIR\${APP_EXE}" "$RestartArguments --installation-check"
  ${Else}
    ExecShell "open" "$INSTDIR\${APP_EXE}" "$RestartArguments"
  ${EndIf}
FunctionEnd

Function .onInit
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/LOG=" $InstallLogPath
  ${IfNot} ${Errors}
    FileOpen $InstallLogFile "$InstallLogPath" w
    ${IfNot} ${Errors}
      FileWriteByte $InstallLogFile 255
      FileWriteByte $InstallLogFile 254
    ${EndIf}
  ${EndIf}
  ${IfNot} ${RunningX64}
    !insertmacro ReportMessage MB_ICONSTOP "Rotor requires 64-bit Windows."
    Abort
  ${EndIf}
  SetRegView 64
  ReadRegStr $R1 HKLM "Software\${REGISTRY_KEY}" "InstallDir"
  ${If} $R1 == ""
    ReadRegStr $R1 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGISTRY_KEY}" "InstallLocation"
  ${EndIf}
  ${If} $R1 != ""
    StrCpy $INSTDIR $R1
  ${EndIf}
  StrCpy $RestartArguments ""
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/PROFILE=" $R1
  ${IfNot} ${Errors}
    StrCpy $R2 $R1 1 -1
    ${If} $R2 == "\"
      StrCpy $R1 "$R1\"
    ${EndIf}
    StrCpy $RestartArguments '--data-dir $\"$R1$\"'
  ${EndIf}
  ClearErrors
  ${GetOptions} $R0 "/NOELEVATE" $R1
  ${IfNot} ${Errors}
    StrCpy $RestartArguments "$RestartArguments --no-elevate"
  ${EndIf}
  ClearErrors
  ${GetOptions} $R0 "/NOINDEX" $R1
  ${IfNot} ${Errors}
    StrCpy $RestartArguments "$RestartArguments --no-index"
  ${EndIf}
  ClearErrors
  ${GetOptions} $R0 "/NOHOTKEYS" $R1
  ${IfNot} ${Errors}
    StrCpy $RestartArguments "$RestartArguments --no-hotkeys"
  ${EndIf}
  ClearErrors
  ${GetOptions} $R0 "/PRODUCTIONSHORTCUTS" $R1
  ${IfNot} ${Errors}
    StrCpy $RestartArguments "$RestartArguments --production-shortcuts"
  ${EndIf}
  ClearErrors
  ${GetOptions} $R0 "/UPDATE" $R1
  ${IfNot} ${Errors}
    ClearErrors
    ${GetOptions} $R0 "/PARENT=" $ParentPid
    ${If} ${Errors}
      ; Tauri 2.6 updater supplies /UPDATE /ARGS, without our /PARENT switch.
      System::Call 'kernel32::OpenMutexW(i 0x100001, i 0, w "${LEGACY_MUTEX}") p.r1'
      ${If} $1 != 0
        System::Call 'kernel32::WaitForSingleObject(p r1, i 30000) i.r2'
        ${If} $2 == 0
        ${OrIf} $2 == 128
          System::Call 'kernel32::ReleaseMutex(p r1)'
        ${EndIf}
        System::Call 'kernel32::CloseHandle(p r1)'
        ${If} $2 != 0
        ${AndIf} $2 != 128
          !insertmacro ReportMessage MB_ICONSTOP "Rotor has not exited. Close it and retry the update."
          Abort
        ${EndIf}
      ${EndIf}
      Return
    ${EndIf}
    ${If} $ParentPid <= 0
      !insertmacro ReportMessage MB_ICONSTOP "Invalid update parent process."
      Abort
    ${EndIf}
    ; Wait for the normal quit path to flush configuration and pin records.
    System::Call 'kernel32::OpenProcess(i 0x100000, i 0, i $ParentPid) p.r1'
    ${If} $1 != 0
      System::Call 'kernel32::WaitForSingleObject(p r1, i 30000) i.r2'
      System::Call 'kernel32::CloseHandle(p r1)'
      ${If} $2 != 0
        !insertmacro ReportMessage MB_ICONSTOP "Rotor has not exited. Close it and retry the update."
        Abort
      ${EndIf}
    ${EndIf}
  ${EndIf}
FunctionEnd

Section "Rotor" SEC_MAIN
  SectionIn RO
  SetShellVarContext all
  ; NSIS GetFullPathName can empty its output for a not-yet-created path.
  ; Normalize lexically before creating the destination, then reject truncation.
  System::Call 'kernel32::GetFullPathNameW(w "$INSTDIR", i ${NSIS_MAX_STRLEN}, w .r0, p 0) i.r1'
  ${If} $1 == 0
  ${OrIf} $1 >= ${NSIS_MAX_STRLEN}
    !insertmacro ReportMessage MB_ICONSTOP "Invalid or excessively long installation directory."
    Abort
  ${EndIf}
  StrCpy $INSTDIR $0
  ${If} $InstallLogFile != ""
    FileWriteUTF16LE $InstallLogFile "Install directory: $INSTDIR$\r$\n"
  ${EndIf}
  ${GetRoot} "$INSTDIR" $R0
  GetFullPathName $R0 "$R0\"
  ${If} $INSTDIR == $R0
  ${OrIf} $INSTDIR == $WINDIR
  ${OrIf} $INSTDIR == $PROGRAMFILES64
  ${OrIf} $INSTDIR == $PROGRAMFILES
    !insertmacro ReportMessage MB_ICONSTOP "Choose a dedicated Rotor installation directory."
    Abort
  ${EndIf}
  ; Refuse nonempty directories belonging to another application.
  StrCpy $R9 0
  ClearErrors
  FindFirst $R7 $R8 "$INSTDIR\*"
  ${IfNot} ${Errors}
    ${Do}
      ${If} $R8 != "."
      ${AndIf} $R8 != ".."
      ${AndIf} $R8 != ""
        StrCpy $R9 1
      ${EndIf}
      FindNext $R7 $R8
    ${LoopUntil} ${Errors}
    FindClose $R7
  ${EndIf}
  ${If} $R9 == 1
    ReadRegStr $R0 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGISTRY_KEY}" "InstallLocation"
    ${If} $R0 != ""
      GetFullPathName $R0 "$R0"
    ${EndIf}
    ${If} $R0 != $INSTDIR
      ${IfNot} ${FileExists} "$INSTDIR\native-build.json"
        !insertmacro ReportMessage MB_ICONSTOP "The selected directory is not a recognized Rotor installation."
        Abort
      ${EndIf}
      ${IfNot} ${FileExists} "$INSTDIR\${APP_EXE}"
        !insertmacro ReportMessage MB_ICONSTOP "The selected directory contains a different build identity."
        Abort
      ${EndIf}
    ${EndIf}
  ${EndIf}
  ${GetParent} "$INSTDIR" $InstallParent
  CreateDirectory "$InstallParent"
  ClearErrors
  GetTempFileName $StagedDirectory "$InstallParent"
  ${If} ${Errors}
    !insertmacro ReportMessage MB_ICONSTOP "Cannot create a staging directory beside Rotor."
    Abort
  ${EndIf}
  Delete "$StagedDirectory"
  CreateDirectory "$StagedDirectory"
  SetOutPath "$StagedDirectory"
  ClearErrors
  File /r "${STAGE_DIR}\*"
  ${If} ${Errors}
    !insertmacro ReportMessage MB_ICONSTOP "Cannot extract Rotor. The previous installation is unchanged."
    Abort
  ${EndIf}
  WriteUninstaller "$StagedDirectory\uninstall.exe"
  ${If} ${Errors}
    !insertmacro ReportMessage MB_ICONSTOP "Cannot prepare the uninstaller. The previous installation is unchanged."
    Abort
  ${EndIf}
  SetOutPath "$TEMP"
  StrCpy $PreviousDirectory ""
  ${If} $R9 == 1
    ReadRegStr $PreviousVersion HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGISTRY_KEY}" "DisplayVersion"
    ClearErrors
    GetTempFileName $PreviousDirectory "$InstallParent"
    ${If} ${Errors}
      !insertmacro ReportMessage MB_ICONSTOP "Cannot reserve a backup directory."
      Abort
    ${EndIf}
    Delete "$PreviousDirectory"
    ClearErrors
    Rename "$INSTDIR" "$PreviousDirectory"
    ${If} ${Errors}
      !insertmacro ReportMessage MB_ICONSTOP "Close all Rotor instances before installing. The previous installation is unchanged."
      Abort
    ${EndIf}
  ${Else}
    ; Remove only an existing empty destination, never its contents.
    RMDir "$INSTDIR"
  ${EndIf}
  ClearErrors
  Rename "$StagedDirectory" "$INSTDIR"
  ${If} ${Errors}
    ${If} $PreviousDirectory != ""
      ClearErrors
      Rename "$PreviousDirectory" "$INSTDIR"
      ${If} ${Errors}
        !insertmacro ReportMessage MB_ICONSTOP "Install failed. The previous installation remains at $PreviousDirectory."
        Abort
      ${EndIf}
    ${EndIf}
    !insertmacro ReportMessage MB_ICONSTOP "Install failed. The previous installation has been retained."
    Abort
  ${EndIf}
  ${If} $PreviousDirectory != ""
    WriteRegStr HKLM "Software\${REGISTRY_KEY}" "PreviousInstallLocation" "$PreviousDirectory"
    WriteRegStr HKLM "Software\${REGISTRY_KEY}" "PreviousVersion" "$PreviousVersion"
    DetailPrint "Previous installation retained at $PreviousDirectory"
  ${EndIf}
  WriteRegStr HKLM "Software\${REGISTRY_KEY}" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGISTRY_KEY}" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGISTRY_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGISTRY_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGISTRY_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGISTRY_KEY}" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGISTRY_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGISTRY_KEY}" "NoRepair" 1
  CreateShortcut "$SMPROGRAMS\${PRODUCT_NAME}.lnk" "$INSTDIR\${APP_EXE}"
SectionEnd

Function un.onInit
  SetRegView 64
  ${un.GetParameters} $R0
  ClearErrors
  ${un.GetOptions} $R0 "/ROLLBACK" $R1
  ${If} ${Errors}
    Return
  ${EndIf}
  SetErrorLevel 1
  ${un.GetOptions} $R0 "/PARENT=" $ParentPid
  ${If} ${Errors}
    !insertmacro ReportMessage MB_ICONSTOP "Rollback requires an update parent process."
    Quit
  ${EndIf}
  ${If} $ParentPid <= 0
    !insertmacro ReportMessage MB_ICONSTOP "Invalid rollback parent process."
    Quit
  ${EndIf}
  ReadRegStr $PreviousDirectory HKLM "Software\${REGISTRY_KEY}" "PreviousInstallLocation"
  ReadRegStr $PreviousVersion HKLM "Software\${REGISTRY_KEY}" "PreviousVersion"
  ${If} $PreviousDirectory == ""
    !insertmacro ReportMessage MB_ICONSTOP "No previous installation is registered."
    Quit
  ${EndIf}
  GetFullPathName $INSTDIR "$INSTDIR"
  ReadRegStr $R1 HKLM "Software\${REGISTRY_KEY}" "InstallDir"
  GetFullPathName $R1 "$R1"
  ${un.GetRoot} "$INSTDIR" $R2
  GetFullPathName $R2 "$R2\"
  ${If} $INSTDIR != $R1
  ${OrIf} $INSTDIR == $R2
  ${OrIf} $INSTDIR == $WINDIR
  ${OrIf} $INSTDIR == $PROGRAMFILES64
  ${OrIf} $INSTDIR == $PROGRAMFILES
    !insertmacro ReportMessage MB_ICONSTOP "Rollback target does not match the registered Rotor directory."
    Quit
  ${EndIf}
  GetFullPathName $PreviousDirectory "$PreviousDirectory"
  ${un.GetParent} "$INSTDIR" $InstallParent
  ${un.GetParent} "$PreviousDirectory" $R1
  ${If} $R1 != $InstallParent
  ${OrIf} $PreviousDirectory == $INSTDIR
    !insertmacro ReportMessage MB_ICONSTOP "Invalid rollback location. No files have been moved."
    Quit
  ${EndIf}
  ${IfNot} ${FileExists} "$PreviousDirectory\${APP_EXE}"
    !insertmacro ReportMessage MB_ICONSTOP "The previous Rotor executable is unavailable."
    Quit
  ${EndIf}
  StrCpy $RestartArguments ""
  ClearErrors
  ${un.GetOptions} $R0 "/PROFILE=" $R1
  ${IfNot} ${Errors}
    StrCpy $R2 $R1 1 -1
    ${If} $R2 == "\"
      StrCpy $R1 "$R1\"
    ${EndIf}
    StrCpy $RestartArguments '--data-dir $\"$R1$\"'
  ${EndIf}
  ClearErrors
  ${un.GetOptions} $R0 "/NOELEVATE" $R1
  ${IfNot} ${Errors}
    StrCpy $RestartArguments "$RestartArguments --no-elevate"
  ${EndIf}
  ClearErrors
  ${un.GetOptions} $R0 "/NOINDEX" $R1
  ${IfNot} ${Errors}
    StrCpy $RestartArguments "$RestartArguments --no-index"
  ${EndIf}
  ClearErrors
  ${un.GetOptions} $R0 "/NOHOTKEYS" $R1
  ${IfNot} ${Errors}
    StrCpy $RestartArguments "$RestartArguments --no-hotkeys"
  ${EndIf}
  ClearErrors
  ${un.GetOptions} $R0 "/PRODUCTIONSHORTCUTS" $R1
  ${IfNot} ${Errors}
    StrCpy $RestartArguments "$RestartArguments --production-shortcuts"
  ${EndIf}
  System::Call 'kernel32::OpenProcess(i 0x100000, i 0, i $ParentPid) p.r1'
  ${If} $1 != 0
    System::Call 'kernel32::WaitForSingleObject(p r1, i 30000) i.r2'
    System::Call 'kernel32::CloseHandle(p r1)'
    ${If} $2 != 0
      !insertmacro ReportMessage MB_ICONSTOP "Rotor has not exited. Previous files remain at $PreviousDirectory."
      Quit
    ${EndIf}
  ${EndIf}
  ClearErrors
  GetTempFileName $FailedDirectory "$InstallParent"
  ${If} ${Errors}
    !insertmacro ReportMessage MB_ICONSTOP "Cannot reserve a recovery directory."
    Quit
  ${EndIf}
  Delete "$FailedDirectory"
  SetOutPath "$TEMP"
  ClearErrors
  Rename "$INSTDIR" "$FailedDirectory"
  ${If} ${Errors}
    !insertmacro ReportMessage MB_ICONSTOP "Cannot move the failed installation. Close Rotor and retry; previous files remain at $PreviousDirectory."
    Quit
  ${EndIf}
  ClearErrors
  Rename "$PreviousDirectory" "$INSTDIR"
  ${If} ${Errors}
    Rename "$FailedDirectory" "$INSTDIR"
    !insertmacro ReportMessage MB_ICONSTOP "Recovery failed. Previous files remain at $PreviousDirectory."
    Quit
  ${EndIf}
  ${If} $PreviousVersion == ""
    ClearErrors
    GetDLLVersion "$INSTDIR\${APP_EXE}" $R0 $R1
    ${IfNot} ${Errors}
      IntOp $R2 $R0 >> 16
      IntOp $R3 $R0 & 0xFFFF
      IntOp $R4 $R1 >> 16
      StrCpy $PreviousVersion "$R2.$R3.$R4"
    ${EndIf}
  ${EndIf}
  ${If} $PreviousVersion != ""
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGISTRY_KEY}" "DisplayVersion" "$PreviousVersion"
  ${EndIf}
  DeleteRegValue HKLM "Software\${REGISTRY_KEY}" "PreviousInstallLocation"
  DeleteRegValue HKLM "Software\${REGISTRY_KEY}" "PreviousVersion"
  WriteRegStr HKLM "Software\${REGISTRY_KEY}" "FailedInstallLocation" "$FailedDirectory"
  ExecShell "open" "$INSTDIR\${APP_EXE}" "$RestartArguments"
  !insertmacro ReportMessage MB_ICONEXCLAMATION "The update could not start. The previous installation was restored. Failed new files are retained at $FailedDirectory."
  SetErrorLevel 0
  Quit
FunctionEnd

Section "Uninstall"
  SetRegView 64
  SetShellVarContext all
  ClearErrors
  Delete "$INSTDIR\${APP_EXE}"
  ${If} ${Errors}
    !insertmacro ReportMessage MB_ICONSTOP "Close Rotor before uninstalling."
    Abort
  ${EndIf}
  ; Only package-owned files, followed by non-recursive empty-directory removal.
  !include "${UNINSTALL_INCLUDE}"
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCT_NAME}"
  StrLen $R1 '$\"$INSTDIR\${APP_EXE}$\"'
  StrCpy $R2 $R0 $R1
  ${If} $R2 == '$\"$INSTDIR\${APP_EXE}$\"'
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCT_NAME}"
  ${EndIf}
  Delete "$INSTDIR\uninstall.exe"
  Delete "$SMPROGRAMS\${PRODUCT_NAME}.lnk"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "Software\${REGISTRY_KEY}"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${REGISTRY_KEY}"
SectionEnd
