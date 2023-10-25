use std::f32::consts::PI;

use glam::{vec3, Mat4, Quat, Vec3};
use stardust_xr_fusion::{
	core::values::Transform,
	drawable::{Model, ModelPart},
	fields::BoxField,
	input::{InputDataType, InputHandler},
	node::{NodeError, NodeType},
	spatial::Spatial,
	HandlerWrapper,
};
use stardust_xr_molecules::input_action::{
	BaseInputAction, InputAction, InputActionHandler, SingleActorAction,
};

pub struct Handle {
	model_part: ModelPart,
	_field: BoxField,
	hold_offset: Vec3,
	input_handler: HandlerWrapper<InputHandler, InputActionHandler<()>>,
	condition_action: BaseInputAction<()>,
	hold_action: SingleActorAction<()>,
}
impl Handle {
	pub async fn create(model: &Model, right: bool) -> Result<Self, NodeError> {
		let model_part = model.model_part(if !right { "Handle_L" } else { "Handle_R" })?;

		// The bones in the center should be driven by the handles themselves so the panel can bend
		model
			.model_part(if !right { "Frame/Left" } else { "Frame/Right" })?
			.set_spatial_parent_in_place(&model_part)?;

		// Make the box field for interaction based on the model itself!
		let (field_pos, field_rot, field_size) = model
			.model_part(if !right {
				"Handle_L/Field_L"
			} else {
				"Handle_R/Field_R"
			})?
			.get_position_rotation_scale(&model_part)?
			.await?;
		let field = BoxField::create(
			&model_part,
			Transform::from_position_rotation(field_pos, field_rot),
			field_size,
		)?;

		let (hold_offset, _, _) = model
			.model_part(if !right {
				"Handle_L/Hold_Center_L"
			} else {
				"Handle_R/Hold_Center_R"
			})?
			.get_position_rotation_scale(&model_part)?
			.await?;

		// And make the input handler so we can hold the panel
		let input_handler =
			InputHandler::create(model.client()?.get_root(), Transform::none(), &field)?
				.wrap(InputActionHandler::default())?;
		let condition_action = BaseInputAction::new(false, |input, _| match &input.input {
			InputDataType::Hand(_) => input.distance < 0.04,
			_ => false,
		});
		let hold_action = SingleActorAction::new(
			true,
			|input, _| {
				input.datamap.with_data(|d| {
					d.idx("pinch_strength").as_f32() > 0.75
						&& d.idx("grab_strength").as_f32() > 0.75
				})
			},
			false,
		);
		Ok(Handle {
			model_part,
			_field: field,
			hold_offset: hold_offset.into(),
			input_handler,
			condition_action,
			hold_action,
		})
	}

	pub fn update_single(&mut self) {
		self.input_handler.lock_wrapped().update_actions([
			self.condition_action.type_erase(),
			self.hold_action.type_erase(),
		]);
		self.hold_action.update(&mut self.condition_action);

		if let Some(holding) = self.hold_action.actor() {
			let InputDataType::Hand(hand) = &holding.input else {return};
			let knuckles: [Vec3; 4] = [
				hand.index.proximal.position.into(),
				hand.little.proximal.position.into(),
				hand.index.distal.position.into(),
				hand.little.distal.position.into(),
			];
			let knuckle_center = knuckles.iter().sum::<Vec3>() / (knuckles.len() as f32);

			let rotation = Quat::from_rotation_arc(
				vec3(0.0, 1.0, 0.0),
				(knuckles[0] - knuckles[1]).normalize(),
			) * Quat::from_rotation_y(PI);
			self.model_part
				.set_transform(
					Some(&self.input_handler.node()),
					Transform::from_position_rotation(
						knuckle_center - (rotation * self.hold_offset),
						rotation,
					),
				)
				.unwrap();
		}
	}

	pub fn update_with_other(&mut self, other: &Handle) {}

	pub fn root(&self) -> &Spatial {
		&self.model_part
	}
}
