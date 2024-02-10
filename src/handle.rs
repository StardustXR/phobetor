use std::f32::consts::PI;

use glam::{Affine3A, Quat, Vec3};
use stardust_xr_fusion::{
	drawable::{Model, ModelPart},
	fields::BoxField,
	input::{InputDataType, InputHandler},
	node::{NodeError, NodeType},
	spatial::{Spatial, SpatialAspect, Transform},
	HandlerWrapper,
};
use stardust_xr_molecules::input_action::{BaseInputAction, InputActionHandler, SingleActorAction};

pub struct GrabInfo {
	position: Vec3,
	direction: Vec3,
}

pub struct Handle {
	root_space: Spatial,
	root: Spatial,
	handle_part: ModelPart,
	right: bool,
	_field: BoxField,
	hold_center: ModelPart,
	grab_info: Option<GrabInfo>,
	input_handler: HandlerWrapper<InputHandler, InputActionHandler<()>>,
	condition_action: BaseInputAction<()>,
	pub hold_action: SingleActorAction<()>,
}
impl Handle {
	pub async fn create(model: Model, right: bool) -> Result<Self, NodeError> {
		let root_space = model.client()?.get_root().alias();
		let handle_part = model.model_part(if !right { "Handle_L" } else { "Handle_R" })?;
		let root = Spatial::create(&root_space, Transform::identity(), false)?;
		root.set_relative_transform(&handle_part, Transform::identity())?;
		handle_part.set_spatial_parent_in_place(&root)?;

		// The bones in the center should be driven by the handles themselves so the panel can bend
		model
			.model_part(if !right { "Frame/Left" } else { "Frame/Right" })?
			.set_spatial_parent_in_place(&handle_part)?;

		// Make the box field for interaction based on the model itself!
		let field_transform = model
			.model_part(if !right {
				"Handle_L/Field_L"
			} else {
				"Handle_R/Field_R"
			})?
			.get_transform(&handle_part)
			.await?;
		let _field = BoxField::create(
			&handle_part,
			Transform::from_translation_rotation(
				field_transform.translation.unwrap(),
				field_transform.rotation.unwrap(),
			),
			field_transform.scale.unwrap(),
		)?;

		// The point that this should be held at
		let hold_center = model.model_part(if !right {
			"Handle_L/Hold_Center_L"
		} else {
			"Handle_R/Hold_Center_R"
		})?;

		// And make the input handler so we can hold the panel
		let input_handler = InputActionHandler::wrap(
			InputHandler::create(&root_space, Transform::none(), &_field)?,
			(),
		)?;

		let condition_action = BaseInputAction::new(false, |input, _| match &input.input {
			InputDataType::Hand(_) => input.distance < 0.025,
			_ => false,
		});
		let hold_action = SingleActorAction::new(
			true,
			|input, _| {
				input
					.datamap
					.with_data(|d| d.idx("grab_strength").as_f32() > 0.75)
			},
			false,
		);
		Ok(Handle {
			root_space,
			root,
			handle_part,
			right,
			_field,
			hold_center,
			grab_info: None,
			input_handler,
			condition_action,
			hold_action,
		})
	}

	pub fn update_single(&mut self) {
		self.input_handler
			.lock_wrapped()
			.update_actions([&mut self.condition_action, self.hold_action.base_mut()]);
		self.hold_action.update(Some(&mut self.condition_action));

		// Makes orientation a TON easier
		if self.hold_action.actor_started() {
			self.move_root(&self.hold_center).unwrap();
		}

		if let Some(holding) = self.hold_action.actor() {
			let InputDataType::Hand(hand) = &holding.input else {
				println!("not a hand :(");
				return;
			};
			let knuckles: [Vec3; 4] = [
				hand.index.proximal.position.into(),
				hand.little.proximal.position.into(),
				hand.middle.distal.position.into(),
				hand.little.distal.position.into(),
			];

			let knuckle_center = knuckles.iter().sum::<Vec3>() / (knuckles.len() as f32);
			let proximal_direction = knuckles[0] - knuckles[1];
			let distal_direction = knuckles[2] - knuckles[3];
			let direction = ((proximal_direction + distal_direction) / 2.0).normalize();
			self.grab_info.replace(GrabInfo {
				position: knuckle_center,
				direction,
			});
		}

		// Makes alignment and closing WAY easier
		if self.hold_action.actor_stopped() {
			self.move_root(&self.handle_part).unwrap();
			self.grab_info.take();
		}
	}

	fn move_root(&self, to: &impl SpatialAspect) -> Result<(), NodeError> {
		self.handle_part
			.set_spatial_parent_in_place(&self.root_space)?;
		self.root
			.set_relative_transform(to, Transform::identity())?;
		self.handle_part.set_spatial_parent_in_place(&self.root)?;
		Ok(())
	}

	pub fn update_with_other(&mut self, other: &Handle) {
		let do_parent_single =
			!self.hold_action.actor_acting() && other.hold_action.actor_started();
		let do_parent_both = self.hold_action.actor_stopped() && other.hold_action.actor_acting();
		let do_unparent_single =
			self.hold_action.actor_started() && other.hold_action.actor_acting();
		let do_unparent_both =
			!self.hold_action.actor_acting() && other.hold_action.actor_stopped();

		if do_parent_single || do_parent_both {
			self.root.set_spatial_parent_in_place(&other.root).unwrap();
		}
		if do_unparent_single || do_unparent_both {
			self.root
				.set_spatial_parent_in_place(&self.root_space)
				.unwrap();
		}

		if let Some(grab_info) = &self.grab_info {
			if let Some(other_grab_info) = &other.grab_info {
				let basis_vector_x = (other_grab_info.position - grab_info.position)
					.reject_from(grab_info.direction)
					.normalize();
				let basis_vector_y = grab_info.direction;
				let basis_vector_z = basis_vector_x.cross(basis_vector_y).normalize();

				let rotation_matrix = Affine3A::from_cols(
					basis_vector_x.into(),
					basis_vector_y.into(),
					basis_vector_z.into(),
					Vec3::ZERO.into(),
				);
				let mut rotation = Quat::from_affine3(&rotation_matrix);
				if !self.right {
					rotation *= Quat::from_rotation_y(PI);
				}

				self.root
					.set_local_transform(Transform::from_translation_rotation(
						grab_info.position,
						rotation,
					))
					.unwrap();
			} else {
				self.root
					.set_local_transform(Transform::from_translation(grab_info.position))
					.unwrap();
			}
		}
	}
}
