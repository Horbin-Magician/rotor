Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
!ifndef STAGE_DIR
!error "Pass /DSTAGE_DIR=<verified native staging directory>"
!endif
!ifndef OUTPUT_FILE
!error "Pass /DOUTPUT_FILE=<installer path>"
!endif
!ifndef APP_VERSION
!error "Pass /DAPP_VERSION=<workspace version>"
!endif
Name "Rotor GPUI Development"
OutFile "${OUTPUT_FILE}"
InstallDir "$PROGRAMFILES64\Rotor GPUI Development"
InstallDirRegKey HKLM "Software\RotorGpuiDevelopment" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "Rotor requires 64-bit Windows."
    Abort
  ${EndIf}
  SetRegView 64
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
  WriteRegStr HKLM "Software\RotorGpuiDevelopment" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RotorGpuiDevelopment" "DisplayName" "Rotor GPUI Development"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RotorGpuiDevelopment" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RotorGpuiDevelopment" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RotorGpuiDevelopment" "DisplayIcon" "$INSTDIR\rotor-desktop.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RotorGpuiDevelopment" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RotorGpuiDevelopment" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RotorGpuiDevelopment" "NoRepair" 1
  CreateShortcut "$SMPROGRAMS\Rotor GPUI Development.lnk" "$INSTDIR\rotor-desktop.exe"
SectionEnd

Section "Uninstall"
  SetRegView 64
  SetShellVarContext all
  ; Only package-owned paths. User profiles are outside the install directory.
  Delete "$INSTDIR\rotor-desktop.exe"
  Delete "$INSTDIR\DirectML.dll"
  Delete "$INSTDIR\resources.json"
  Delete "$INSTDIR\native-app.toml"
  Delete "$INSTDIR\update-public.key"
  RMDir /r "$INSTDIR\assets"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$SMPROGRAMS\Rotor GPUI Development.lnk"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "Software\RotorGpuiDevelopment"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RotorGpuiDevelopment"
SectionEnd
