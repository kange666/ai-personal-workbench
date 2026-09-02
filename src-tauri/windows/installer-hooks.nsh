; “星枢工作台”曾在 V1.9.0/V1.9.1 被误用作 NSIS 产品身份，
; 导致从“AI 个人工作台”升级时切换到新的默认安装目录。
; 新版本恢复旧产品身份，并在安装前移除误建的重复安装。
; /UPDATE 会保留 com.local.ai-personal-workbench 下的用户数据。
!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $R8 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\星枢工作台" "UninstallString"
  ${If} $R8 != ""
    DetailPrint "正在合并重复的星枢工作台安装..."
    ClearErrors
    ExecWait '$R8 /UPDATE /P' $R9
    ${If} ${Errors}
      DetailPrint "未能启动重复安装的卸载程序，将继续更新原安装目录。"
    ${ElseIf} $R9 != 0
      DetailPrint "重复安装清理返回代码 $R9，将继续更新原安装目录。"
    ${EndIf}
  ${EndIf}
!macroend
