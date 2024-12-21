searchState.loadedDescShard("sbi", 0, "This crate provides a safe, pure-Rust implementation of …\nThe resource is already available\nThe resource was previously started\nThe resource was previously stopped\nThe SBI implementation has denied execution of the call …\nThe SBI call failed\nA SBI hart mask\nAn invalid address was passed\nAn invalid parameter was passed\nThe SBI call is not implemented or the functionality is …\nError codes returned by SBI calls\nRequired base SBI functionality\nA zero-argument <code>ecall</code> with the given extension and …\nA one-argument <code>ecall</code> with the given extension and function …\nA two-argument <code>ecall</code> with the given extension and function …\nA three-argument <code>ecall</code> with the given extension and …\nA four-argument <code>ecall</code> with the given extension and …\nA five-argument <code>ecall</code> with the given extension and …\nA six-argument <code>ecall</code> with the given extension and function …\nReturns the argument unchanged.\nCreate a new <code>HartMask</code> from the given hart ID, making it …\nReturns the argument unchanged.\nA convenience macro to help create a <code>HartMask</code> from either …\nHart State Management extension\nA convenience alias to the <code>hart_state_management</code> module.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nIPI extension\nLegacy SBI calls\nCreate a new <code>HartMask</code> with the given base and no hart IDs …\nPerformance Monitoring Unit extension\nRFENCE extension\nSystem Reset extension\nTimer extension\nSelect the given hart ID. If <code>hart_id</code> is out of the range …\nThe extension is available, along with its …\nBase extension ID\nExtension availability information returned by …\nSBI implementation name\nSBI specification version implemented by the SBI …\nThe extension is unavailable\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nRetrieve the SBI implementation ID\nRetrieve the SBI implementation’s version. The encoding …\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nConvert to the <code>usize</code> implementation ID value\nHelper method for converting <code>ExtensionAvailability</code> to a …\nMajor version number\nRetrieve the value of the <code>marchid</code> CSR\nRetrieve the value of the <code>mimpid</code> CSR\nMinor version number\nRetrieve the value of <code>mvendorid</code> CSR\nProbe the availability of the extension ID <code>id</code>\nRetrieve the SBI specification version\nDefault non-retentive suspension which does not save any …\nDefault retentive suspension which saves register and CSR …\nHart state management extension ID\nExecution status for a hart\nA platform specific non-retentive suspend type, which does …\nA platform specific retentive suspend type, which saves …\nAn event has caused the hart to begin resuming normal …\nA start request is pending for the hart\nThe hart is powered on and executing normally\nA stop request is pending for the hart\nThe hart is currently not executing in supervisor mode or …\nA suspend request is pending for the hart\nThe type of suspension to be executed whe ncalling …\nThe hart is in a platform specific suspend (or low power) …\nReturns the argument unchanged.\nReturns the argument unchanged.\nStart the specific hart ID at the given physical address …\nRetrieve the status of the specified hart ID.\nThis SBI call stops S-mode execution on the current hart …\nPlaces the current hart into a suspended or low power …\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nUser-defined opaque value passed to <code>resume_address</code> in <code>a1</code> …\nUser-defined opaque value passed to <code>resume_address</code> in <code>a1</code> …\nThe address to resume execution at.\nThe address to resume execution at.\nThe platform-specific suspend value, in the range …\nThe IPI extension ID\nSend an inter-processor interrupt (IPI) to the harts …\n<code>sbi_clear_ipi</code> extension ID\n<code>sbi_console_getchar</code> extension ID\n<code>sbi_console_putchar</code> extension ID\n<code>sbi_remote_fence_i</code> extension ID\n<code>sbi_remote_sfence_vma_asid</code> extension ID\n<code>sbi_remote_sfence_vma</code> extension ID\n<code>sbi_send_ipi</code> extension ID\n<code>sbi_set_timer</code> extension ID\n<code>sbi_shutdown</code> extension ID\nClears any pending interprocessor interrupts (IPIs) for …\nAttempt to retrieve a character from the debug console. If …\nWrite a character to the debug console. This call will …\nExecute a <code>FENCE.I</code> instruction on the harts specified by …\nExecute a <code>SFENCE.VMA</code> instruction on the harts specified by …\nExecute a <code>SFENCE.VMA</code> instruction on the harts specified by …\nSend an interprocessor interrupt (IPI) to all of the harts …\nSchedule an interrupt for <code>time</code> in the future. To clear the …\nPuts all harts into a shutdown state wherein the execution …\nStart the counter after configuring it\nClear (or zero) the counter value\nCounter configuration flags\nA logical index assigned to a specific performance counter\nA bitmask of counter indices to be acted upon\nInformation about a specific performance counter\nCounter start flags\nCounter stop flags\nData translation lookaside buffer cache\nPerformance Monitoring Unit extension ID\nA specific performance monitoring event in an <code>EventType</code>\nA hardware or firmware event type\nA type of performance monitoring event\nThe counter is a firmware provided performance counter\nA firmware performance monitoring event type\nFirmware performance monitoring event metrics\nThe counter is a hardware performance counter\nA hardware cache performance monitoring event type\nA hardware cache performance monitoring event code\nThe hardware cache unit to monitor\nThe cache operation to monitor\nThe result of the caching operation\nA general hardware performance monitoring event type\nA general hardware performance monitoring event code\nA raw hardware performance monitoring event\nA raw hardware performance monitoring event code\nInstruction translation lookaside buffer cache\nLast level cache\nFirst level data cache\nFirst level instruction cache\nMore verbose name for <code>Self::SET_MINH</code>. Hints to the SBI …\nNo flags\nNo flags\nNo flags\nNon-uniform memory access node cache\nReset the counter to event mapping\nSet the initial counter value\nHints to the SBI implementation to inhibit event counting …\nHints to the SBI implementation to inhibit event counting …\nHints to the SBI implementation to inhibit event counting …\nHints to the SBI implementation to inhibit event counting …\nHints to the SBI implementation to inhibit event counting …\nSkip the counter matching\nMore verbose name for <code>Self::SET_SINH</code>. Hints to the SBI …\nMore verbose name for <code>Self::SET_UINH</code>. Hints to the SBI …\nMore verbose name for <code>Self::SET_VSINH</code>. Hints to the SBI …\nMore verbose name for <code>Self::SET_VUINH</code>. Hints to the SBI …\nConfigure a set of matching performance counters described …\nRetreive the information associated with a given …\nCreates a new <code>CounterIndexMask</code> with a base value of <code>0</code> and …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCreate a new <code>CounterIndexMask</code> from the given <code>CounterIndex</code>, …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCreate a new <code>CounterIndexMask</code> with the given base and no …\nCreate a new <code>CounterIndex</code>\nCreate a new <code>EventIndex</code> from the given <code>EventType</code> and …\nCreate a new <code>HardwareCacheEventCode</code> from the cache unit, …\nReturns the number of available performance counters, both …\nRead the current value of the specified <code>CounterIndex</code>.\nStart the performance counters described by the given …\nStop the performance counters described by the given …\nSelect the given counter index. If <code>counter_idx</code> is out of …\nThe underlying CSR number backing the performance counter\nThe CSR width. Equal to one less than the number of the …\nThe RFENCE extension ID\nInstructs the given harts to execute a <code>FENCE.I</code> instruction.\nInstructs the given harts to execute a <code>HFENCE.GVMA</code> for the …\nInstructs the given harts to execute a <code>HFENCE.GVMA</code> for the …\nInstructs the given harts to execute a <code>HFENCE.VVMA</code> for the …\nInstructs the given harts to execute a <code>HFENCE.VVMA</code> for the …\nInstructs the given harts to execute a <code>SFENCE.VMA</code> for the …\nInstructs the given harts to execute a <code>SFENCE.VMA</code> for the …\nPower off all hardware and perform a cold boot\nSystem reset extension ID\nNo reason for reset\nPlatform specific reset type. The variant value is a value …\nPlatform specific reset reason. The variant value is a …\nThe reason for performing the reset\nThe type of reset to perform\nSBI implementation specific reset reason. The variant …\nShutdown the system\nSystem failure\nReset processors and some hardware\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nAttempt to reset the system in the provided method, with a …\nTimer extension ID\nSchedule an interrupt for <code>time</code> in the future. To clear the …")