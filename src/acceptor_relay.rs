use stardust_xr_fusion::items::{
	panel::{PanelItem, PanelItemInitData},
	ItemAcceptorHandler,
};

pub struct CapturedItem {
	uid: String,
	_item: PanelItem,
}

pub struct AcceptorRelay(tokio::sync::watch::Sender<Option<CapturedItem>>);
impl ItemAcceptorHandler<PanelItem> for AcceptorRelay {
	fn captured(&mut self, uid: String, item: PanelItem, _init_data: PanelItemInitData) {
		self.0
			.send(Some(CapturedItem { uid, _item: item }))
			.unwrap();
	}
	fn released(&mut self, uid: String) {
		self.0.send_if_modified(|c| {
			let remove = c.as_mut().is_some_and(|c| c.uid == uid);
			if remove {
				c.take();
			}
			remove
		});
	}
}
