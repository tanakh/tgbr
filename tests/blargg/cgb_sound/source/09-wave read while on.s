; Reads from wave RAM while playing, each time 2
; clocks later.

;.define REQUIRE_DMG 1
;.define REQUIRE_CGB 1
.include "shell.inc"
.include "apu.s"

main:
     wreg NR51,0    ; mute sound
     loop_n_times test,69
     check_crc_dmg_cgb $118A3620,$270DA9A3
     jp   tests_passed

test:
     add  $99
     ld   b,a
     
     ; Reload wave and have its first
     ; sample read occur 2 clocks earlier
     ; each loop iteration
     ld   hl,wave
     call load_wave
     wreg NR30,$80  ; enable
     wreg NR32,$00  ; silent
     ld   a,b
     sta  NR33      ; period
     wreg NR34,$87  ; start
     
     ; Read from wave
     wreg NR33,-2   ; period = 4
     delay_clocks 176
     lda  WAVE
     
     call print_a
     
     ret

wave:
     .byte $00,$11,$22,$33,$44,$55,$66,$77
     .byte $88,$99,$AA,$BB,$CC,$DD,$EE,$FF
