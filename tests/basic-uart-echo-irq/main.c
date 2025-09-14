#include <8051.h>
#include <stdint.h>
#include "rtl837x_sfr.h"

// #define REGDBG 1
// #define RXTXDBG 1


#define SYS_TICK_HZ 100
#define SERIAL_BAUD_RATE 57600
#define CLOCK_HZ 125000000

// Derive the divider settings for the internal clock
#if CLOCK_HZ == 20800000
#define CLOCK_DIV 3
#elif CLOCK_HZ == 31250000
#define CLOCK_DIV 2
#elif CLOCK_HZ == 62500000
#define CLOCK_DIV 1
#elif CLOCK_HZ == 125000000
#define CLOCK_DIV 0
#endif

volatile uint8_t ticks;



void isr_timer0(void) __interrupt(1)
{
	TR0 = 0;		// Stop timer 0
	TH0 = (0x10000 - (CLOCK_HZ / SYS_TICK_HZ / 32)) >> 8;
	TL0 = (0x10000 - (CLOCK_HZ / SYS_TICK_HZ / 32)) % 0xff;
	TR0 = 1;		// Re-start timer 0

	ticks++;
}

void write_char(char c)
{
	do {
	} while (TI == 0);
	TI = 0;
	SBUF = c;
}


void print_string(__code char *p)
{
	while (*p)
		write_char(*p++);
}


void setup_serial(void)
{
	IE = 0;

	T2CON = 0x34; // Enable RCLK/TCLK (serial transmit/receive clock for T2), TR2 (Timer 2 RUN), disable CP/RL2 (bit 0)
	SCON = 0x50;  // Mode = 1: ASYNC 8N1 with T2 as baud-rate generator, REN_0 Receive enable

	// The RCAP2 registers contain the high/low byte that is loaded into
	// timer2 when T2 overflows to 0x10000
	RCAP2H = (0x10000 - (CLOCK_HZ / SERIAL_BAUD_RATE / 32)) >> 8;
	RCAP2L = (0x10000 - (CLOCK_HZ / SERIAL_BAUD_RATE / 32)) % 0xff;

	PCON |= 0x80; // Double the Baud Rate

	SCON = 0x50;
	TI = 1;
	RI = 0;

	ES = 1; // Enable serial IRQ
}


void installer(void)
{
    // Disable all interrupts (global and individually) by setting IE register (SFR A8) to 0
	IE = 0;
	EIE = 0;  // SFR e8: EIE. Disable all external IRQs

	// Disable all interrupts (global interrupt enable bit)
	EA = 0; // SFR A8.7 / IE.7


	SBUF = 0xAA;

	write_char('I');

	// setup_serial();
	while (1) {
		PSW.1

		uint8_t t = (v / 100);
		// when print_zeros is not zero, we know that a non-zero number has printed.
		// That have to print all the next numbers.
		uint8_t print_zeros = t;
		if (print_zeros)
			char_to_html('0' + t);
		t = (v / 10) % 10;
		print_zeros |= t;
		if (print_zeros)
			char_to_html('0' + t);
		char_to_html('0' + (v % 10));

	}
		// print_string("Image installer running\n");
}
