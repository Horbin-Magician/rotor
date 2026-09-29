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
; Keep native, keyboard-accessible controls and let MUI scale artwork with DPI.
ManifestDPIAware true
ManifestSupportedOS all
BrandingText "${PRODUCT_NAME}  ${APP_VERSION}"
!define MUI_ICON "..\assets\icons\icon.ico"
!define MUI_UNICON "..\assets\icons\icon.ico"
!define MUI_WELCOMEFINISHPAGE_BITMAP "installer\windows-sidebar.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "installer\windows-sidebar.bmp"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "installer\windows-header.bmp"
!define MUI_HEADERIMAGE_UNBITMAP "installer\windows-header.bmp"
!define MUI_BGCOLOR FFFFFF
!define MUI_TEXTCOLOR 18354B
!define MUI_INSTFILESPAGE_COLORS "18354B F4F8FB"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "$(RotorWelcomeTitle)"
!define MUI_WELCOMEPAGE_TEXT "$(RotorWelcomeText)"
!define MUI_WELCOMEPAGE_TITLE_3LINES
!define MUI_FINISHPAGE_TITLE "$(RotorFinishTitle)"
!define MUI_FINISHPAGE_TEXT "$(RotorFinishText)"
!define MUI_FINISHPAGE_TITLE_3LINES
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "$(RotorRunText)"
!define MUI_FINISHPAGE_RUN_FUNCTION RestartRotor
!insertmacro MUI_PAGE_WELCOME
!define MUI_PAGE_HEADER_TEXT "$(RotorDirectoryTitle)"
!define MUI_PAGE_HEADER_SUBTEXT "$(RotorDirectorySubtitle)"
!define MUI_DIRECTORYPAGE_TEXT_TOP "$(RotorDirectoryText)"
!insertmacro MUI_PAGE_DIRECTORY
!define MUI_PAGE_HEADER_TEXT "$(RotorInstallingTitle)"
!define MUI_PAGE_HEADER_SUBTEXT "$(RotorInstallingSubtitle)"
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

LangString RotorWelcomeTitle ${LANG_ENGLISH} "Welcome to ${PRODUCT_NAME}"
LangString RotorWelcomeText ${LANG_ENGLISH} "Your desktop, within reach.$\r$\n$\r$\nFind files, capture and pin screenshots, and translate text in one place.$\r$\n$\r$\nClose Rotor before continuing, then click Next to choose where to install."
LangString RotorDirectoryTitle ${LANG_ENGLISH} "Choose a home for Rotor"
LangString RotorDirectorySubtitle ${LANG_ENGLISH} "Select the installation folder."
LangString RotorDirectoryText ${LANG_ENGLISH} "Use the folder below or click Browse to choose another location.$\r$\n$\r$\nYour settings and saved pins are stored separately from the application."
LangString RotorInstallingTitle ${LANG_ENGLISH} "Setting up Rotor"
LangString RotorInstallingSubtitle ${LANG_ENGLISH} "Please wait while the application files are installed."
LangString RotorFinishTitle ${LANG_ENGLISH} "${PRODUCT_NAME} is ready"
LangString RotorFinishText ${LANG_ENGLISH} "Installation is complete.$\r$\n$\r$\nOpen Rotor to get started. You can also find it in the Start menu."
LangString RotorRunText ${LANG_ENGLISH} "Open ${PRODUCT_NAME}"
LangString RotorWelcomeTitle ${LANG_SIMPCHINESE} "欢迎使用 ${PRODUCT_NAME}"
LangString RotorWelcomeText ${LANG_SIMPCHINESE} "让桌面，得心应手。$\r$\n$\r$\n查找文件、截图贴图、翻译文字，$\r$\n日常工具，一处就绪。$\r$\n$\r$\n请先退出正在运行的 Rotor，$\r$\n再点击「下一步」选择安装位置。"
LangString RotorDirectoryTitle ${LANG_SIMPCHINESE} "为 Rotor 选择安装位置"
LangString RotorDirectorySubtitle ${LANG_SIMPCHINESE} "选择用于存放应用程序的文件夹。"
LangString RotorDirectoryText ${LANG_SIMPCHINESE} "使用下方的默认位置，或点击「浏览」选择其他文件夹。$\r$\n$\r$\n设置和已保存的贴图会独立存放在用户配置目录中。"
LangString RotorInstallingTitle ${LANG_SIMPCHINESE} "正在安装 Rotor"
LangString RotorInstallingSubtitle ${LANG_SIMPCHINESE} "正在准备应用文件，请稍候。"
LangString RotorFinishTitle ${LANG_SIMPCHINESE} "${PRODUCT_NAME} 已准备就绪"
LangString RotorFinishText ${LANG_SIMPCHINESE} "安装已完成。$\r$\n$\r$\n打开 Rotor，即可开始使用。你也可以在开始菜单中找到它。"
LangString RotorRunText ${LANG_SIMPCHINESE} "打开 ${PRODUCT_NAME}"

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

; Antivirus/indexing can briefly retain extracted files after their writers close.
; Bound retries; a genuinely locked installation still fails without losing data.
!macro RenameWithRetry SOURCE DESTINATION
  StrCpy $R6 0
  ${Do}
    ClearErrors
    Rename "${SOURCE}" "${DESTINATION}"
    ${IfNot} ${Errors}
      ${ExitDo}
    ${EndIf}
    IntOp $R6 $R6 + 1
    ${If} $R6 >= 50
      SetErrors
      ${ExitDo}
    ${EndIf}
    Sleep 100
  ${Loop}
!macroend

Function RestartRotor
  ${If} $PreviousDirectory != ""
  ${AndIf} ${FileExists} "$PreviousDirectory\${APP_EXE}"
    ExecShell "open" "$INSTDIR\rotor-recovery.exe" "$RestartArguments"
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
  ${GetOptions} $R0 "/BACKGROUND" $R1
  ${IfNot} ${Errors}
    StrCpy $RestartArguments "$RestartArguments --background"
  ${EndIf}
  ClearErrors
  ${GetOptions} $R0 "/UPDATE" $R1
  ${IfNot} ${Errors}
    ClearErrors
    ${GetOptions} $R0 "/PARENT=" $ParentPid
    ${If} ${Errors}
      !insertmacro ReportMessage MB_ICONSTOP "Native updates require /PARENT=<process id>."
      Abort
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
    !insertmacro RenameWithRetry "$INSTDIR" "$PreviousDirectory"
    ${If} ${Errors}
      !insertmacro ReportMessage MB_ICONSTOP "Close all Rotor instances before installing. The previous installation is unchanged."
      Abort
    ${EndIf}
  ${Else}
    ; Remove only an existing empty destination, never its contents.
    RMDir "$INSTDIR"
  ${EndIf}
  ClearErrors
  !insertmacro RenameWithRetry "$StagedDirectory" "$INSTDIR"
  ${If} ${Errors}
    ${If} $PreviousDirectory != ""
      ClearErrors
      !insertmacro RenameWithRetry "$PreviousDirectory" "$INSTDIR"
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
  ClearErrors
  ${un.GetOptions} $R0 "/BACKGROUND" $R1
  ${IfNot} ${Errors}
    StrCpy $RestartArguments "$RestartArguments --background"
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
