use std::sync::Arc;
use glam::{vec3, vec4, Mat4};
use crate::renderer::gpu_storage::{GpuStorage, GpuStorable};
use crate::renderer::types::light::Light;
use crate::renderer::types::shadow_data::ShadowData;
use crate::renderer::types::light_type::LightType;
use crate::renderer::shadow_data_storage::ShadowDataStorage;
use crate::renderer::light_storage::LightStorage;
use crate::renderer::types::camera::Camera;
use shipyard::Unique;
use crate::renderer::shadow_atlas::ShadowAtlas;

/// The LightManager encapsulates both light and shadow storage, along with update logic.
#[derive(Unique)]
pub struct LightManager {
    pub light_storage: LightStorage,
    pub shadow_data_storage: ShadowDataStorage,
}

impl LightManager {
    /// Creates a new LightManager using the given device and queue.
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let light_storage = LightStorage::new(device.clone(), queue.clone());
        let shadow_data_storage = ShadowDataStorage::new(device, queue);
        Self {
            light_storage,
            shadow_data_storage,
        }
    }

    /// Creates a directional light.
    pub fn create_directional_light(&mut self, shadow_atlas: &mut ShadowAtlas) -> Light {
        let position = vec3(0.0, 5.0, 0.0);
        // For a directional light we use a direction vector rather than a rotation,
        // but if your API expects a rotation (as in your current Light struct) you might
        // pass the direction directly.
        let direction = vec3(0.5, -1.0, 0.0);
        let color = vec3(1.0, 1.0, 1.0);
        let intensity = 0.5;
        let range = 100.0;
        let spot_angle = 0.0; // Not used.
        let mut light = Light::new(position, direction, color, intensity, range, spot_angle, LightType::Directional);

        // Allocate one tile for shadow mapping.
        let tile = shadow_atlas
            .allocate_tile(2048, 2048)
            .expect("Failed to allocate directional shadow tile");
        let shadow_data = ShadowData::new(
            Mat4::IDENTITY, // Will be updated each frame.
            tile.read().unwrap().uv_offset,
            tile.read().unwrap().uv_scale,
            0.000015,
        );
        let smc = crate::renderer::ecs::components::ShadowMapComponent::new(shadow_data, tile);
        let idx = self.shadow_data_storage.add_shadow_data(smc);
        light.set_shadow_data(idx as u32, 1);
        light
    }

    /// Creates a point light with given parameters.
    pub fn create_point_light(
        &mut self,
        position: glam::Vec3,
        color: glam::Vec3,
        intensity: f32,
        range: f32,
        shadow_atlas: &mut ShadowAtlas,
    ) -> Light {
        let rotation = vec3(0.0, 0.0, 0.0);
        let mut light = Light::new(position, rotation, color, intensity, range, 0.0, LightType::Point);

        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.1, range);
        let views = [
            Mat4::look_at_rh(position, position + vec3(1.0, 0.0, 0.0), vec3(0.0, -1.0, 0.0)),
            Mat4::look_at_rh(position, position + vec3(-1.0, 0.0, 0.0), vec3(0.0, -1.0, 0.0)),
            Mat4::look_at_rh(position, position + vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0)),
            Mat4::look_at_rh(position, position + vec3(0.0, -1.0, 0.0), vec3(0.0, 0.0, -1.0)),
            Mat4::look_at_rh(position, position + vec3(0.0, 0.0, 1.0), vec3(0.0, -1.0, 0.0)),
            Mat4::look_at_rh(position, position + vec3(0.0, 0.0, -1.0), vec3(0.0, -1.0, 0.0)),
        ];

        let shadow_map_resolution = 1024;
        let mut start_index: u32 = 0;
        for i in 0..6 {
            let tile = shadow_atlas
                .allocate_tile(shadow_map_resolution, shadow_map_resolution)
                .expect("Failed to allocate point light shadow tile");
            let shadow_data = ShadowData::new(
                proj * views[i],
                tile.read().unwrap().uv_offset,
                tile.read().unwrap().uv_scale,
                0.000005,
            );
            let smc = crate::renderer::ecs::components::ShadowMapComponent::new(shadow_data, tile);
            let idx = self.shadow_data_storage.add_shadow_data(smc);
            if i == 0 {
                start_index = idx as u32;
            }
        }
        light.set_shadow_data(start_index, 6);
        light
    }

    /// Creates a spotlight with given parameters.
    pub fn create_spot_light(
        &mut self,
        position: glam::Vec3,
        direction: glam::Vec3,
        color: glam::Vec3,
        intensity: f32,
        range: f32,
        spot_angle: f32,
        shadow_atlas: &mut ShadowAtlas,
    ) -> Light {
        let mut light = Light::new(position, direction, color, intensity, range, spot_angle, LightType::Spot);
        let proj = Mat4::perspective_rh(spot_angle * 2.0, 1.0, 0.01, range);
        let view = Mat4::look_at_rh(position, position + direction, vec3(0.0, 1.0, 0.0));
        let tile = shadow_atlas
            .allocate_tile(512, 512)
            .expect("Failed to allocate spotlight shadow tile");
        let shadow_data = ShadowData::new(
            proj * view,
            tile.read().unwrap().uv_offset,
            tile.read().unwrap().uv_scale,
            0.000005,
        );
        let smc = crate::renderer::ecs::components::ShadowMapComponent::new(shadow_data, tile);
        let idx = self.shadow_data_storage.add_shadow_data(smc);
        light.set_shadow_data(idx as u32, 1);
        light
    }

    /// Updates all lights with respect to the current camera.
    /// This function iterates over the lights, dispatching the update logic based on type,
    /// then batches the updated lights and shadow data to the GPU.
    pub fn update_lights(&mut self, lights: &mut [Light], camera: &Box<dyn Camera>) {
        for light in lights.iter_mut() {
            match LightType::from_u32(light.light_type) {
                LightType::Directional => {
                    Self::update_directional_light(light, camera, &mut self.shadow_data_storage)
                },
                LightType::Point => {
                    Self::update_point_light(light, &mut self.shadow_data_storage)
                },
                LightType::Spot => {
                    Self::update_spot_light(light, &mut self.shadow_data_storage)
                },
                _ => {}
            }
        }
        // Batch update the GPU storage.
        self.light_storage.set_all_lights(lights.to_vec());
        self.light_storage.update();
        self.shadow_data_storage.update();
    }

    fn update_directional_light(light: &mut Light, camera: &Box<dyn Camera>, shadow_storage: &mut ShadowDataStorage) {
        let camera_pos = camera.position();
        let light_dir = -light.rotation.normalize();
        let scene_center = camera_pos;
        let scene_center = vec3(scene_center.x, 5.0, scene_center.z);
        let light_distance = -50.0;
        let light_pos = scene_center - light_dir * light_distance;
        let light_view = Mat4::look_at_rh(light_pos, scene_center, glam::Vec3::Y);
        let left = -30.0;
        let right = 30.0;
        let bottom = -30.0;
        let top = 30.0;
        let near = 0.1;
        let far = 100.0;
        let proj = Mat4::orthographic_rh(left, right, bottom, top, near, far);
        let mut light_view_proj = proj * light_view;

        // Update each shadow tile.
        for i in 0..light.shadow_data_count {
            if let Some(mut shadow_map_data) = shadow_storage.get_shadow_data((light.shadow_data_offset + i) as usize) {
                shadow_map_data.shadow_data.light_view_proj = light_view_proj;
                shadow_storage.set_shadow_data((light.shadow_data_offset + i) as usize, shadow_map_data).unwrap();
            }
        }
        light.view_proj = light_view_proj;
    }

    fn update_point_light(light: &mut Light, shadow_storage: &mut ShadowDataStorage) {
        let light_pos = light.position;
        let proj = glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.01, light.range);
        let views = [
            Mat4::look_at_rh(light_pos, light_pos + vec3(1.0, 0.0, 0.0), vec3(0.0, -1.0, 0.0)),
            Mat4::look_at_rh(light_pos, light_pos + vec3(-1.0, 0.0, 0.0), vec3(0.0, -1.0, 0.0)),
            Mat4::look_at_rh(light_pos, light_pos + vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0)),
            Mat4::look_at_rh(light_pos, light_pos + vec3(0.0, -1.0, 0.0), vec3(0.0, 0.0, -1.0)),
            Mat4::look_at_rh(light_pos, light_pos + vec3(0.0, 0.0, 1.0), vec3(0.0, -1.0, 0.0)),
            Mat4::look_at_rh(light_pos, light_pos + vec3(0.0, 0.0, -1.0), vec3(0.0, -1.0, 0.0)),
        ];
        for i in 0..light.shadow_data_count {
            if let Some(mut shadow_map_data) = shadow_storage.get_shadow_data(light.shadow_data_offset as usize + i as usize) {
                let view_proj = proj * views[i as usize];
                shadow_map_data.shadow_data.light_view_proj = view_proj;
                shadow_storage.set_shadow_data(light.shadow_data_offset as usize + i as usize, shadow_map_data).unwrap();
            }
        }
        light.view_proj = Mat4::IDENTITY;
    }

    fn update_spot_light(light: &mut Light, shadow_storage: &mut ShadowDataStorage) {
        let light_pos = light.position;
        let light_dir = light.rotation.normalize();
        let fov = if light.spot_angle > 0.0 { light.spot_angle * 2.0 } else { std::f32::consts::FRAC_PI_4 };
        let aspect = 1.0;
        let near = 0.01;
        let far = light.range;
        let proj = Mat4::perspective_rh(fov, aspect, near, far);
        let view = Mat4::look_at_rh(light_pos, light_pos + light_dir, glam::Vec3::Y);
        let view_proj = proj * view;
        for i in 0..light.shadow_data_count {
            if let Some(mut shadow_map_data) = shadow_storage.get_shadow_data((light.shadow_data_offset + i) as usize) {
                shadow_map_data.shadow_data.light_view_proj = view_proj;
                shadow_storage.set_shadow_data((light.shadow_data_offset + i) as usize, shadow_map_data).unwrap();
            }
        }
        light.view_proj = view_proj;
    }
}
