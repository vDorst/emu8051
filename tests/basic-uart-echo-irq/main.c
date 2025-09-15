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



uint8_t rx_data = 0;
__sbit rx_new_data = 0;

void isr_serial(void) __interrupt(1)
{
	if (RI) {
		RI = 0;
		rx_new_data = 1;
		rx_data = SBUF;
		// SBUF = rx_data;
	}
	if (TI) {
		TI = 0;
	}
}

void write_char(char c)
{
	SBUF = c;
}

void setup_serial(void)
{
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

	ES = 1; // Enable serial IRQ
}

void main(void)
{
	// Disable all interrupts (global and individually) by setting IE register (SFR A8) to 0
	IE = 0;
	EIE = 0;  // SFR e8: EIE. Disable all external IRQs

	setup_serial();

	// Disable all interrupts (global interrupt enable bit)
	EA = 1; // SFR A8.7 / IE.7

	while (1) {
		// put in idle, wait for serial
		if (rx_new_data) {
			ES = 0;
			rx_new_data = 0;
			SBUF = rx_data;
			ES = 1;
		} else {
			PCON |= 1;
		}
	}
}
