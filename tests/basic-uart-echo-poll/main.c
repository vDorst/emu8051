#include <8051.h>
#include <stdint.h>
#include "rtl837x_sfr.h"

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

void write_char(char c)
{
	SBUF = c;
	do {
	} while (TI == 0);
	TI = 0;
}

char read_char(void) {
	do {
	} while (RI == 0);
	RI = 0;
	return SBUF;
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
	TI = 0;
	RI = 0;

	ES = 0; // Enable serial IRQ
}

void main(void)
{
    // Disable all interrupts (global and individually) by setting IE register (SFR A8) to 0
	IE = 0;
	EIE = 0;  // SFR e8: EIE. Disable all external IRQs

	// Disable all interrupts (global interrupt enable bit)
	EA = 0; // SFR A8.7 / IE.7

	uint8_t c = 0xA5;

	while (1) {
		c = read_char();
		write_char(c);
	}
}
