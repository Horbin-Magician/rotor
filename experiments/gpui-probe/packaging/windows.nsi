!include "MUI2.nsh"
!ifndef PROBE_EXE
!error "Pass /DPROBE_EXE=absolute-path-to-release-exe"
!endif
!ifndef OUTPUT_FILE
!error "Pass /DOUTPUT_FILE=absolute-path-to-installer"
!endif
Unicode true
Name "Rotor GPUI P0"
OutFile "${OUTPUT_FILE}"
InstallDir "$PROGRAMFILES64\Rotor GPUI P0"
RequestExecutionLevel admin
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
Section
  SetRegView 64
  SetOutPath "$INSTDIR"
  File "${PROBE_EXE}"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RotorGpuiP0" "DisplayName" "Rotor GPUI P0"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RotorGpuiP0" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
SectionEnd
Section "Uninstall"
  SetRegView 64
  Delete "$INSTDIR\rotor-gpui-probe.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RotorGpuiP0"
SectionEnd
