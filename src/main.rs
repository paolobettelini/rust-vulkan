use std::time::{Duration, Instant};
use std::thread::sleep;

use vulkanalia::{
    loader::{LibloadingLoader, LIBRARY},
    window as vk_window,
    prelude::v1_0::*,
    vk::{KhrSurfaceExtension, KhrSwapchainExtension}
};

use winit::{
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
    event::{Event, WindowEvent},
    dpi::LogicalSize,
    platform::unix::WindowExtUnix
};

use std::collections::HashSet;

const DEVICE_EXTENSIONS: &[vk::ExtensionName] = &[vk::KHR_SWAPCHAIN_EXTENSION.name];

fn main() {
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Rusty Vulkan")
        .with_inner_size(LogicalSize::new(1024, 768))
        .build(&event_loop).unwrap();

    let mut app = App::new(window);

    let fps: u32 = 60;
    let frame_interval = Duration::new(0, 1000000000u32 / fps);
    let mut frame_count = 0;

    let mut destroying = false;
    let mut i = 0;
    event_loop.run(move |event, _, control_flow| {
        let frame_start = Instant::now();

        *control_flow = ControlFlow::Poll;

        match event {
            // Render a frame if our Vulkan app is not being destroyed.
            Event::MainEventsCleared if !destroying => {
                println!("{frame_count}");
                app.render();
            }
            // Destroy our Vulkan app.
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                destroying = true;
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }

        // Limit to FPS
        frame_count += 1;
        let delta = frame_start.elapsed();
        if delta < frame_interval {
            sleep(frame_interval - delta);
        }
    });

}

struct App {
    surface: vk::SurfaceKHR,
    window: Window,
    entry: Entry,
    instance: Instance,
    device: Device,
    physical_device: vk::PhysicalDevice,
    queue_container: QueueContainer,
    swapchain: vk::SwapchainKHR,
    swapchain_format: vk::Format,
    swapchain_extent: vk::Extent2D,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>
}

impl App {
    
    fn new(window: Window) -> Self {
        unsafe {
            let loader = LibloadingLoader::new(LIBRARY).unwrap();
            let entry = Entry::new(loader).unwrap();
            let instance = create_instance(&window, &entry);
            let surface = vk_window::create_surface(&instance, &window).unwrap();
            let physical_device = pick_physical_device(&instance, surface);
            let (device, queue_container) = create_logical_device(&instance, physical_device, surface);
            let (swapchain,
                swapchain_format,
                swapchain_extent) = create_swapchain(&window, &instance, &device, physical_device, surface);
            let swapchain_images = device.get_swapchain_images_khr(swapchain).unwrap();
            let swapchain_image_views = create_swapchain_image_views(&device, &swapchain_images, swapchain_format);

            Self {
                surface: surface,
                window: window,
                entry: entry,
                instance: instance,
                device: device,
                physical_device,
                queue_container: queue_container,
                swapchain: swapchain,
                swapchain_format: swapchain_format,
                swapchain_extent: swapchain_extent,
                swapchain_images: swapchain_images,
                swapchain_image_views: swapchain_image_views
            }
        }
    }

    fn render(&mut self) {

    }

}

unsafe fn create_swapchain(window: &Window, instance: &Instance, device: &Device, physical_device: vk::PhysicalDevice, surface: vk::SurfaceKHR)
        -> (vk::SwapchainKHR, vk::Format, vk::Extent2D) {
    let (graphics_index, present_index) = get_queues_indices(instance, physical_device, surface); // TODO do this once and pass them. Also for support ?
    let support = SwapchainSupport::new(&instance, physical_device, surface);

    let surface_format = get_swapchain_surface_format(&support.formats);
    let present_mode = get_swapchain_present_mode(&support.present_modes);
    let extent = get_swapchain_extent(window, support.capabilities);

    let mut image_count = support.capabilities.min_image_count + 1;
    
    if support.capabilities.max_image_count != 0 && image_count > support.capabilities.max_image_count {
        image_count = support.capabilities.max_image_count;
    }
    println!("Imagine count {image_count}");

    let mut queue_family_indices = vec![];
    let image_sharing_mode = if graphics_index != present_index {
        queue_family_indices.push(graphics_index);
        queue_family_indices.push(present_index);
        vk::SharingMode::CONCURRENT // specify in advanced the queue families.
    } else {
        vk::SharingMode::EXCLUSIVE
    };

    let info = vk::SwapchainCreateInfoKHR::builder()
        .surface(surface)
        .min_image_count(image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(image_sharing_mode)
        .queue_family_indices(&queue_family_indices)
        .pre_transform(support.capabilities.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped(true)
        .old_swapchain(vk::SwapchainKHR::null());
    
    let swapchain = device.create_swapchain_khr(&info, None).unwrap();

    (swapchain, surface_format.format, extent)
}

unsafe fn create_swapchain_image_views(device: &Device, images: &Vec<vk::Image>, swapchain_format: vk::Format) -> Vec<vk::ImageView> {
    images
        .iter()
        .map(|image| {
            let components = vk::ComponentMapping::builder()
                .r(vk::ComponentSwizzle::IDENTITY)
                .g(vk::ComponentSwizzle::IDENTITY)
                .b(vk::ComponentSwizzle::IDENTITY)
                .a(vk::ComponentSwizzle::IDENTITY);

            let subresource_range = vk::ImageSubresourceRange::builder()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1);

            let info = vk::ImageViewCreateInfo::builder()
                .image(*image)
                .view_type(vk::ImageViewType::_2D)
                .format(swapchain_format)
                .components(components)
                .subresource_range(subresource_range);

            device.create_image_view(&info, None)
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
            self.instance.destroy_surface_khr(self.surface, None);
            self.instance.destroy_instance(None);
            self.device.destroy_swapchain_khr(self.swapchain, None);
            self.swapchain_image_views
                .iter()
                .for_each(|view| self.device.destroy_image_view(*view, None));
        }
    }
}

/// The queue for graphics operations might differ
/// from the queue for presenting on the window.
struct QueueContainer {
    graphics_index: u32,
    present_index: u32,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
}

impl QueueContainer {
    fn new(graphics_index: u32, present_index: u32, graphics_queue: vk::Queue, present_queue: vk::Queue) -> Self {
        Self {
            graphics_index,
            present_index,
            graphics_queue,
            present_queue
        }
    }
}

struct SwapchainSupport {
    capabilities: vk::SurfaceCapabilitiesKHR,
    formats: Vec<vk::SurfaceFormatKHR>,
    present_modes: Vec<vk::PresentModeKHR>,
}

impl SwapchainSupport {
    fn new(instance: &Instance, physical_device: vk::PhysicalDevice, surface: vk::SurfaceKHR) -> Self {
        unsafe {
            Self {
                capabilities: instance
                    .get_physical_device_surface_capabilities_khr(
                        physical_device, surface).unwrap(),
                formats: instance
                    .get_physical_device_surface_formats_khr(
                        physical_device, surface).unwrap(),
                present_modes: instance
                    .get_physical_device_surface_present_modes_khr(
                        physical_device, surface).unwrap(),
            }
        }
    }
}

unsafe fn create_instance(window: &Window, entry: &Entry) -> Instance {
    let application_info = vk::ApplicationInfo::builder()
        .application_name(b"Vulkan Tutorial\0")
        .application_version(vk::make_version(1, 0, 0))
        .engine_name(b"No Engine\0")
        .engine_version(vk::make_version(1, 0, 0))
        .api_version(vk::make_version(1, 0, 0));

    let extensions = vk_window::get_required_instance_extensions(window)
        .iter()
        .map(|e| e.as_ptr())
        .collect::<Vec<_>>();

    #[cfg(debug_assertions)] { // if DEBUG_MODE
        let available_layers = entry
            .enumerate_instance_layer_properties()
            .unwrap()
            .iter()
            .map(|l| l.layer_name)
            .collect::<HashSet<_>>();
        
        let validation_layer = vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");
        
        if !available_layers.contains(&validation_layer) {
            panic!("Validation layer requested not supported. You may need to install them");
        }
    
        let layers = vec![validation_layer.as_ptr()];

        let info = vk::InstanceCreateInfo::builder()
            .application_info(&application_info)
            .enabled_layer_names(&layers)
            .enabled_extension_names(&extensions);
 
        entry.create_instance(&info, None).unwrap()
    }

    #[cfg(not(debug_assertions))] { // if RELEASE_MODE
        let info = vk::InstanceCreateInfo::builder()
            .application_info(&application_info)
            .enabled_layer_names(&layers)
            .enabled_extension_names(&extensions);

        entry.create_instance(&info, None).unwrap()
    }
}

unsafe fn pick_physical_device(instance: &Instance, surface: vk::SurfaceKHR) -> vk::PhysicalDevice {
    for physical_device in instance.enumerate_physical_devices().unwrap() {
        let properties = instance.get_physical_device_properties(physical_device);
    
        // Check device type

        if properties.device_type != vk::PhysicalDeviceType::DISCRETE_GPU {
            continue;
        }

        // Check features

        let features = instance.get_physical_device_features(physical_device);
        
        if features.geometry_shader != vk::TRUE {
            continue;
        }

        // Check extensions support

        let extensions = instance
            .enumerate_device_extension_properties(physical_device, None)
            .unwrap()
            .iter()
            .map(|e| e.extension_name)
            .collect::<HashSet<_>>();

        if !DEVICE_EXTENSIONS.iter().all(|e| extensions.contains(e)) {
            continue;
        }

        // Check swapchain support for surface

        let support = SwapchainSupport::new(instance, physical_device, surface);
        if support.formats.is_empty() || support.present_modes.is_empty() {
            continue;
        }
        
        println!("Selected physical device (`{}`).", properties.device_name);
        return physical_device;
    }

    panic!("Failed to find suitable physical device.");
}


unsafe fn create_logical_device(instance: &Instance, physical_device: vk::PhysicalDevice, surface: vk::SurfaceKHR) -> (Device, QueueContainer) {
    // Queue Create Infos

    let (graphics_index, present_index) = get_queues_indices(instance, physical_device, surface);

    let mut unique_indices = HashSet::new(); // useless?
    unique_indices.insert(graphics_index);
    unique_indices.insert(present_index);

    let queue_priorities = &[1.0];
    let queue_infos = unique_indices
        .iter()
        .map(|i| {
            vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(*i)
                .queue_priorities(queue_priorities)
        })
        .collect::<Vec<_>>();

    // Layers

    //let layers = vec![VALIDATION_LAYER.as_ptr()];

    // Features

    let features = vk::PhysicalDeviceFeatures::builder();

    // Create

    let extensions = DEVICE_EXTENSIONS
        .iter()
        .map(|n| n.as_ptr())
        .collect::<Vec<_>>();

    let info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_infos)
        //.enabled_layer_names(&layers)
        .enabled_extension_names(&extensions)
        .enabled_features(&features);

    let device = instance
        .create_device(physical_device, &info, None)
        .unwrap();

    // Queues

    let graphics_queue = device.get_device_queue(graphics_index, 0);
    let present_queue = device.get_device_queue(present_index, 0);

    let container = QueueContainer::new(graphics_index, present_index, graphics_queue, present_queue);

    (device, container)
}

fn get_queues_indices(instance: &Instance, physical_device: vk::PhysicalDevice, surface: vk::SurfaceKHR) -> (u32, u32) {
    unsafe {
        let properties = instance.get_physical_device_queue_family_properties(physical_device);
        
        let graphics = properties
        .iter()
        .position(|p| p.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        .map(|i| i as u32);
        
        let mut present;
        for (index, _properties) in properties.iter().enumerate() {
            let index = index as u32;
            
            if instance.get_physical_device_surface_support_khr(physical_device, index, surface).unwrap() {
                present = Some(index);
                
                if let (Some(graphics), Some(index)) = (graphics, present) {
                    return (graphics, index);
                }
            }
        }
    }

    panic!("Missing required queue families.");
}

fn get_swapchain_surface_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
    formats
        .iter()
        .cloned()
        .find(|f| {
            f.format == vk::Format::B8G8R8A8_SRGB
                && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR // qui
        })
        .unwrap_or_else(|| formats[0])
}

fn get_swapchain_present_mode(present_modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    /*present_modes
        .iter()
        .cloned()
        .find(|m| *m == vk::PresentModeKHR::MAILBOX)
        .unwrap_or(vk::PresentModeKHR::FIFO)*/
    vk::PresentModeKHR::FIFO
}

fn get_swapchain_extent(window: &Window, capabilities: vk::SurfaceCapabilitiesKHR) -> vk::Extent2D {
    if capabilities.current_extent.width != u32::max_value() {
        capabilities.current_extent
    } else {
        let size = window.inner_size();
        let clamp = |min: u32, max: u32, v: u32| min.max(max.min(v));
        vk::Extent2D::builder()
            .width(clamp(
                capabilities.min_image_extent.width,
                capabilities.max_image_extent.width,
                size.width,
            ))
            .height(clamp(
                capabilities.min_image_extent.height,
                capabilities.max_image_extent.height,
                size.height,
            ))
            .build()
    }
}

unsafe fn create_pipeline(device: &Device) -> () {
    
}