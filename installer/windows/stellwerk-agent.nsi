; Installer for the Stellwerk agent on Windows: per user, no admin rights.
; Installs both programs, pairs the device (with a code, or in the LAN without one) and sets
; up the task at logon through `stellwerk-agent service install`.
;
; makensis /DVERSION=1.0.0 /DBIN=<dir with both .exe> /DOUT=<setup.exe> stellwerk-agent.nsi
; Silent: setup.exe /S /SERVER=https://sync.example.org /CODE=K7Q2-MX9P-41TZ
; Behind a private CA also /CASHA256=<fingerprint> (`stellwerk-agent join --ca-sha256`).

Unicode true
!include "MUI2.nsh"
!include "nsDialogs.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif

Name "Stellwerk Agent"
OutFile "${OUT}"
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\Programs\Stellwerk"
SetCompressor /SOLID lzma

!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\StellwerkAgent"

Var Server
Var Code
Var CaArg
Var ServerField
Var CodeField

!insertmacro MUI_PAGE_LICENSE "${LICENSE}"
Page custom PairPage PairPageLeave
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "German"
!insertmacro MUI_LANGUAGE "English"

LangString PairTitle ${LANG_GERMAN} "Mit der Zentrale koppeln"
LangString PairTitle ${LANG_ENGLISH} "Pair with the Zentrale"
LangString PairText ${LANG_GERMAN} "Adresse der Zentrale und der Code aus „Gerät koppeln“. Ohne Code wartet das Gerät auf Freigabe in der Zentrale."
LangString PairText ${LANG_ENGLISH} "The Zentrale's address and the code from “Pair device”. Without a code the device waits for approval in the Zentrale."
LangString ServerLabel ${LANG_GERMAN} "Adresse, z. B. https://sync.example.org"
LangString ServerLabel ${LANG_ENGLISH} "Address, e.g. https://sync.example.org"
LangString CodeLabel ${LANG_GERMAN} "Code (leer lassen für Koppeln im LAN)"
LangString CodeLabel ${LANG_ENGLISH} "Code (leave empty to pair in the LAN)"
LangString ServerMissing ${LANG_GERMAN} "Bitte die Adresse der Zentrale angeben."
LangString ServerMissing ${LANG_ENGLISH} "Please enter the Zentrale's address."
LangString Failed ${LANG_GERMAN} "Fehlgeschlagen:"
LangString Failed ${LANG_ENGLISH} "Failed:"

Function .onInit
  ${GetParameters} $0
  Push $0
  Push "/SERVER="
  Call ParameterValue
  Pop $Server
  Push $0
  Push "/CODE="
  Call ParameterValue
  Pop $Code
  Push $0
  Push "/CASHA256="
  Call ParameterValue
  Pop $CaArg
  ${If} $CaArg != ""
    StrCpy $CaArg ' --ca-sha256 "$CaArg"'
  ${EndIf}
FunctionEnd

; The value after NAME in the parameters, up to the next space. GetOptions from FileFunc
; would stop at the next "/" and cut https://… short.
; Stack in: parameters, NAME (top). Stack out: the value, empty when NAME is missing.
Function ParameterValue
  Exch $R1
  Exch
  Exch $R0
  Push $R2
  Push $R3
  Push $R4
  StrLen $R2 $R1
  StrCpy $R3 0
  find:
    StrCpy $R4 $R0 $R2 $R3
    StrCmp $R4 "" missing
    StrCmp $R4 $R1 found
    IntOp $R3 $R3 + 1
    Goto find
  found:
    IntOp $R3 $R3 + $R2
    StrCpy $R0 $R0 "" $R3
    StrCpy $R3 0
  scan:
    StrCpy $R4 $R0 1 $R3
    StrCmp $R4 "" cut
    StrCmp $R4 " " cut
    IntOp $R3 $R3 + 1
    Goto scan
  cut:
    StrCpy $R0 $R0 $R3
    Goto done
  missing:
    StrCpy $R0 ""
  done:
  Pop $R4
  Pop $R3
  Pop $R2
  Exch
  Pop $R1
  Exch $R0
FunctionEnd

Function PairPage
  !insertmacro MUI_HEADER_TEXT "$(PairTitle)" "$(PairText)"
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateLabel} 0 0 100% 12u "$(ServerLabel)"
  Pop $0
  ${NSD_CreateText} 0 14u 100% 12u "$Server"
  Pop $ServerField
  ${NSD_CreateLabel} 0 36u 100% 12u "$(CodeLabel)"
  Pop $0
  ${NSD_CreateText} 0 50u 100% 12u "$Code"
  Pop $CodeField
  nsDialogs::Show
FunctionEnd

Function PairPageLeave
  ${NSD_GetText} $ServerField $Server
  ${NSD_GetText} $CodeField $Code
  ${If} $Server == ""
    MessageBox MB_ICONEXCLAMATION "$(ServerMissing)"
    Abort
  ${EndIf}
FunctionEnd

; Runs the agent with arguments; on failure shows its output, keeps it in install-error.log
; and aborts the installation.
!macro Agent ARGS
  nsExec::ExecToStack '"$INSTDIR\stellwerk-agent.exe" ${ARGS}'
  Pop $0
  Pop $1
  DetailPrint $1
  ${If} $0 != 0
    FileOpen $2 "$INSTDIR\install-error.log" w
    FileWrite $2 $1
    FileClose $2
    MessageBox MB_ICONSTOP "$(Failed)$\r$\n$1" /SD IDOK
    SetErrorLevel 2
    Abort
  ${EndIf}
!macroend

Section "Agent"
  SetOutPath "$INSTDIR"
  ; A running older agent holds its files: stop it first.
  ${If} ${FileExists} "$INSTDIR\stellwerk-agent.exe"
    nsExec::ExecToLog '"$INSTDIR\stellwerk-agent.exe" service uninstall'
    Pop $0
  ${EndIf}
  File "${BIN}\stellwerk-agent.exe"
  File "${BIN}\stellwerk-agent-background.exe"
  File "${LICENSE}"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ${If} $Code == ""
    !insertmacro Agent 'join --server "$Server"$CaArg'
  ${Else}
    !insertmacro Agent 'join --server "$Server"$CaArg "$Code"'
  ${EndIf}
  !insertmacro Agent 'service install'

  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "Stellwerk Agent"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "Stellwerk"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

; Stops the agent and its Syncthing and removes the programs. The data directory (key,
; Syncthing database) stays, so reinstalling keeps the pairing; the synced folders are
; never touched.
Section "Uninstall"
  nsExec::ExecToLog '"$INSTDIR\stellwerk-agent.exe" service uninstall'
  Pop $0
  Delete "$INSTDIR\stellwerk-agent.exe"
  Delete "$INSTDIR\stellwerk-agent-background.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\install-error.log"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
