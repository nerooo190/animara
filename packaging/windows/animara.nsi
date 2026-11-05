; NSIS installer (.exe) for Animara.
; Build: makensis /DVERSION=0.1.0 /DBINDIR=..\..\target\release /DOUTFILE=..\..\dist\Animara-0.1.0-windows-x64-setup.exe packaging\windows\animara.nsi
Unicode true
!include "MUI2.nsh"
!ifndef VERSION
  !define VERSION "0.1.0"
!endif
!ifndef BINDIR
  !define BINDIR "..\..\target\release"
!endif
!ifndef OUTFILE
  !define OUTFILE "Animara-${VERSION}-setup.exe"
!endif
Name "Animara ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\Animara"
InstallDirRegKey HKLM "Software\Animara" "InstallDir"
RequestExecutionLevel admin
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Animara"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "CompanyName" "Animara contributors"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Animara (required)" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${BINDIR}\animara.exe"
  File "${BINDIR}\animara-cli.exe"
  WriteRegStr HKLM "Software\Animara" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\Animara" "Version" "${VERSION}"
  ; File association for .anim
  WriteRegStr HKCR ".anim" "" "Animara.Document"
  WriteRegStr HKCR "Animara.Document" "" "Animara animation document"
  WriteRegStr HKCR "Animara.Document\DefaultIcon" "" "$INSTDIR\animara.exe,0"
  WriteRegStr HKCR "Animara.Document\shell\open\command" "" '"$INSTDIR\animara.exe" "%1"'
  ; Uninstaller + Apps & Features entry
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Animara" "DisplayName" "Animara"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Animara" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Animara" "Publisher" "Animara contributors"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Animara" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Animara" "DisplayIcon" "$INSTDIR\animara.exe"
  CreateDirectory "$SMPROGRAMS\Animara"
  CreateShortcut "$SMPROGRAMS\Animara\Animara.lnk" "$INSTDIR\animara.exe"
  CreateShortcut "$SMPROGRAMS\Animara\Uninstall Animara.lnk" "$INSTDIR\uninstall.exe"
SectionEnd

Section "Desktop shortcut" SecDesktop
  CreateShortcut "$DESKTOP\Animara.lnk" "$INSTDIR\animara.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\animara.exe"
  Delete "$INSTDIR\animara-cli.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Animara\Animara.lnk"
  Delete "$SMPROGRAMS\Animara\Uninstall Animara.lnk"
  RMDir "$SMPROGRAMS\Animara"
  Delete "$DESKTOP\Animara.lnk"
  DeleteRegKey HKCR "Animara.Document"
  DeleteRegValue HKCR ".anim" ""
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Animara"
  DeleteRegKey HKLM "Software\Animara"
SectionEnd
