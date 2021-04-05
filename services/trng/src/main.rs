#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

mod api;

use num_traits::FromPrimitive;

use log::info;


#[cfg(target_os = "none")]
mod implementation {
    use utralib::generated::*;
    // use crate::api::*;
    use log::info;

    pub struct Trng {
        csr: utralib::CSR<u32>,
        // TODO: allocate a software buffer for whitened TRNGs
    }

    impl Trng {
        pub fn new() -> Trng {
            let csr = xous::syscall::map_memory(
                xous::MemoryAddress::new(utra::trng_server::HW_TRNG_SERVER_BASE),
                None,
                4096,
                xous::MemoryFlags::R | xous::MemoryFlags::W,
            )
            .expect("couldn't map TRNG CSR range");

            let mut trng = Trng {
                csr: CSR::new(csr.as_mut_ptr() as *mut u32),
            };

            ///// configure power settings and which generator to use
            trng.csr.wo(utra::trng_server::CONTROL,
                trng.csr.ms(utra::trng_server::CONTROL_ENABLE, 1)
                | trng.csr.ms(utra::trng_server::CONTROL_POWERSAVE, 1)
               // | self.server_csr.ms(utra::trng_server::CONTROL_AV_DIS, 1)  // disable the AV generator to characterize the RO
               // | self.server_csr.ms(utra::trng_server::CONTROL_RO_DIS, 1)  // disable the RO to characterize only the AV
            );

            /*** TRNG tuning parameters: these were configured and tested in a long run against Dieharder
                 There is a rate of TRNG generation vs. quality trade-off. The tuning below is toward quality of
                 TRNG versus rate of TRNG, such that we could use these without any whitening.
             ***/
            ///// configure avalanche
            // delay in microseconds for avalanche poweron after powersave
            trng.csr.wo(utra::trng_server::AV_CONFIG,
                trng.csr.ms(utra::trng_server::AV_CONFIG_POWERDELAY, 50_000)
                | trng.csr.ms(utra::trng_server::AV_CONFIG_SAMPLES, 32)
            );

            ///// configure ring oscillator
            trng.csr.wo(utra::trng_server::RO_CONFIG,
                trng.csr.ms(utra::trng_server::RO_CONFIG_DELAY, 4)
                | trng.csr.ms(utra::trng_server::RO_CONFIG_DWELL, 100)
                | trng.csr.ms(utra::trng_server::RO_CONFIG_GANG, 1)
                | trng.csr.ms(utra::trng_server::RO_CONFIG_FUZZ, 1)
                | trng.csr.ms(utra::trng_server::RO_CONFIG_OVERSAMPLING, 3)
            );

            info!("hardware initialized");

            trng
        }

        pub fn get_data_eager(&self) -> u32 {
            while self.csr.rf(utra::trng_server::STATUS_AVAIL) == 0 {
                xous::yield_slice();
            }
            self.csr.rf(utra::trng_server::DATA_DATA)
        }

        #[allow(dead_code)]
        pub fn wait_full(&self) {
            while self.csr.rf(utra::trng_server::STATUS_FULL) == 0 {
                xous::yield_slice();
            }
        }

        pub fn get_trng(&self, count: usize) -> [u32; 2] {
            // TODO: use SHA hardware unit to robustify the TRNG output against potential hardware failures
            // TODO: health monitoring of raw TRNG output
            let mut ret: [u32; 2] = [0, 0];

            /*
               in the final implementation the algorithm should be:
                 1) check fullness of software-whitened pool
                 2) if software pool is full enough, return values from there
                 3) if pool is low, activate hardware TRNG and refill the pool (uses SHA unit)
                 4) during pool-filling, perform statistics on the hardware TRNG output to check health
                 5) confirm health is OK
            */

            // for now, we just take data directly from the hardware-managed raw TRNG pool
            ret[0] = self.get_data_eager();
            // we don't just draw down TRNGs if not requested, because they are a finite resource
            if count > 1 {
                ret[1] = self.get_data_eager();
            }

            ret
        }
    }
}

// a stub to try to avoid breaking hosted mode for as long as possible.
#[cfg(not(target_os = "none"))]
mod implementation {
    use log::info;

    pub struct Trng {
        seed: u32,
    }

    impl Trng {
        pub fn new() -> Trng {
            Trng {
                seed: 0x1afe_cafe,
            }
        }

        fn move_lfsr(&self, mut lfsr: u32) -> u32 {
            lfsr ^= lfsr >> 7;
            lfsr ^= lfsr << 9;
            lfsr ^= lfsr >> 13;
            lfsr
        }

        #[allow(dead_code)]
        pub fn wait_full(&self) { }

        pub fn get_trng(&mut self, _count: usize) -> [u32; 2] {
            info!("hosted mode TRNG is *not* random, it is an LFSR");
            let mut ret: [u32; 2] = [0; 2];
            self.seed = self.move_lfsr(self.seed);
            ret[0] = self.seed;
            self.seed = self.move_lfsr(self.seed);
            ret[1] = self.seed;

            ret
        }
    }
}

#[xous::xous_main]
fn xmain() -> ! {
    use crate::implementation::Trng;

    log_server::init_wait().unwrap();
    info!("my PID is {}", xous::process::id());

    let xns = xous_names::XousNames::new().unwrap();
    let trng_sid = xns.register_name(api::SERVER_NAME_TRNG).expect("can't register server");
    info!("registered with NS -- {:?}", trng_sid);

    #[cfg(target_os = "none")]
    let trng = Trng::new();

    #[cfg(not(target_os = "none"))]
    let mut trng = Trng::new();

    info!("ready to accept requests");

    loop {
        let msg = xous::receive_message(trng_sid).unwrap();
        match FromPrimitive::from_usize(msg.body.id()) {
            Some(api::Opcode::GetTrng) => xous::msg_blocking_scalar_unpack!(msg, count, _, _, _, {
                let val: [u32; 2] = trng.get_trng(count);
                xous::return_scalar2(msg.sender, val[0] as _, val[1] as _)
                    .expect("couldn't return GetTrng request");
            }),
            None => {
                log::error!("couldn't convert opcode");
                break
            }
        }
    }
    // clean up our program
    log::trace!("main loop exit, destroying servers");
    xns.unregister_server(trng_sid).unwrap();
    xous::destroy_server(trng_sid).unwrap();
    log::trace!("quitting");
    xous::terminate_process(); loop {}
}
