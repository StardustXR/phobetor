use std::f32::consts::PI;

use glam::{vec3, Affine3A, Mat3, Mat4, Quat, Vec3};
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

struct GrabInfo {
	position: Vec3,
	direction: Vec3,
}

pub struct Handle {
	model: Model,
	handle_part: ModelPart,
	right: bool,
	_field: BoxField,
	hold_center: ModelPart,
	grab_info: Option<GrabInfo>,
	input_handler: HandlerWrapper<InputHandler, InputActionHandler<()>>,
	condition_action: BaseInputAction<()>,
	hold_action: SingleActorAction<()>,
}
impl Handle {
	pub async fn create(model: Model, right: bool) -> Result<Self, NodeError> {
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

		// The point that this should be held at
		let hold_center = model.model_part(if !right {
			"Handle_L/Hold_Center_L"
		} else {
			"Handle_R/Hold_Center_R"
		})?;

		// And make the input handler so we can hold the panel
		let input_handler =
			InputHandler::create(model.client()?.get_root(), Transform::none(), &field)?
				.wrap(InputActionHandler::default())?;
		let condition_action = BaseInputAction::new(false, |input, _| match &input.input {
			InputDataType::Hand(_) => input.distance < 0.1,
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
			model,
			handle_part: model_part,
			right,
			_field: field,
			hold_center,
			grab_info: None,
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

		// Makes orientation a TON easier
		if self.hold_action.actor_started() {
			self.hold_center
				.set_spatial_parent_in_place(&self.model)
				.unwrap();
			self.handle_part
				.set_spatial_parent_in_place(&self.hold_center)
				.unwrap();
		}

		if let Some(holding) = self.hold_action.actor() {
			let InputDataType::Hand(hand) = &holding.input else {println!("not a hand :("); return};
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
			self.handle_part
				.set_spatial_parent_in_place(&self.model)
				.unwrap();
			self.hold_center
				.set_spatial_parent_in_place(&self.handle_part)
				.unwrap();
			self.grab_info.take();
		}
	}

	pub fn update_with_other(&mut self, other: &Handle) {
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

				self.hold_center
					.set_transform(
						Some(self.input_handler.node()),
						Transform::from_position_rotation(grab_info.position, rotation),
					)
					.unwrap();
			} else {
				self.hold_center
					.set_position(Some(self.input_handler.node()), grab_info.position)
					.unwrap();
			}
		}
	}
	pub fn root(&self) -> &Spatial {
		&self.handle_part
	}
}
