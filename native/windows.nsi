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

Function RestartRotor
  ExecShell "open" "$INSTDIR\${APP_EXE}" "$RestartArguments"
FunctionEnd

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "Rotor requires 64-bit Windows."
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
          MessageBox MB_ICONSTOP "Rotor has not exited. Close it and retry the update."
          Abort
        ${EndIf}
      ${EndIf}
      Return
    ${EndIf}
    ${If} $ParentPid <= 0
      MessageBox MB_ICONSTOP "Invalid update parent process."
      Abort
    ${EndIf}
    ; Wait for the normal quit path to flush configuration and pin records.
    System::Call 'kernel32::OpenProcess(i 0x100000, i 0, i $ParentPid) p.r1'
    ${If} $1 != 0
      System::Call 'kernel32::WaitForSingleObject(p r1, i 30000) i.r2'
      System::Call 'kernel32::CloseHandle(p r1)'
      ${If} $2 != 0
        MessageBox MB_ICONSTOP "Rotor has not exited. Close it and retry the update."
        Abort
      ${EndIf}
    ${EndIf}
  ${EndIf}
FunctionEnd

Section "Rotor" SEC_MAIN
  SectionIn RO
  SetShellVarContext all
  SetOutPath "$INSTDIR"
  ClearErrors
  File /r "${STAGE_DIR}\*"
  IfErrors 0 +3
    MessageBox MB_ICONSTOP "Cannot replace Rotor files. Close Rotor and retry."
    Abort
  WriteUninstaller "$INSTDIR\uninstall.exe"
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

Section "Uninstall"
  SetRegView 64
  SetShellVarContext all
  ClearErrors
  Delete "$INSTDIR\${APP_EXE}"
  ${If} ${Errors}
    MessageBox MB_ICONSTOP "Close Rotor before uninstalling."
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
