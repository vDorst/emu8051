;--------------------------------------------------------
; Stack segment in internal ram
;--------------------------------------------------------
 	.area VECTOR    (CODE)
	.globl __interrupt_vect
__interrupt_vect:
 	ljmp	__sdcc_gsinit_startup
	.ds     7
	reti
	.ds     7
	reti
	.ds     7
	reti
	.ds     7
	reti
	.ds     7
	reti			; 0x2b TIMER 2 IRQ
	.ds     7
	reti			; 0x33 NOT used by DW8051
	.ds     7
	reti			; 0x3b Serial port 1 RX/TX IRQ
	.ds     7
	reti
	.ds     7
	reti

	.globl __start__stack

	.area GSINIT0 (CODE)

__sdcc_gsinit_startup::
        mov     sp,#__start__stack - 1

	.area GSFINAL (CODE)
        ljmp	_main

__sdcc_banked_call::
	push	_PSBANK
	xch	a,r0
	push	a
	mov	a,r1
	push	a
	mov	a,r2
	anl	a,#0x1f
	mov	_PSBANK, a
	xch	a, r0
	ret

__sdcc_banked_ret::
	pop	_PSBANK
	ret

