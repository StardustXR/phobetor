mod acceptor_relay;
pub mod handle;
mod phobetor;

use crate::phobetor::Phobetor;
use color_eyre::eyre::Result;
use manifest_dir_macros::directory_relative_path;
use stardust_xr_fusion::client::Client;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	color_eyre::install()?;
	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let _wrapped = client.wrap_root(Phobetor::new(&client).await?)?;

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e??,
	}
	Ok(())
}
