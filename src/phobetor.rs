use crate::handle::Handle;
use color_eyre::eyre::Result;
use stardust_xr_fusion::{
	client::{Client, ClientState, FrameInfo, RootHandler},
	core::values::ResourceID,
	drawable::Model,
	node::{NodeError, NodeType},
	spatial::Transform,
};
use std::sync::Arc;

pub struct Phobetor {
	_model: Model,
	handles: (Handle, Handle),
}
impl Phobetor {
	pub async fn new(client: &Arc<Client>) -> Result<Self, NodeError> {
		let model = Model::create(
			client.get_root(),
			Transform::identity(),
			&ResourceID::new_namespaced("phobetor", "phobetor"),
		)?;
		let handles = (
			Handle::create(model.alias(), false).await?,
			Handle::create(model.alias(), true).await?,
		);
		Ok(Phobetor {
			_model: model,
			handles,
		})
	}
}
impl RootHandler for Phobetor {
	fn frame(&mut self, _info: FrameInfo) {
		self.handles.0.update_single();
		self.handles.1.update_single();

		self.handles.0.update_with_other(&self.handles.1);
		self.handles.1.update_with_other(&self.handles.0);
	}
	fn save_state(&mut self) -> ClientState {
		// TODO: proper state saving (there's a lot).
		ClientState::default()
	}
}
