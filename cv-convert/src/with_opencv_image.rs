use crate::image;
use crate::opencv::{core as cv, prelude::*};
use crate::with_opencv::MatExt;
use crate::{OpenCvElement, TryToCv};
use anyhow::{bail, ensure, Error, Result};
use std::ops::Deref;
use cv::DataType;

// &ImageBuffer -> Mat
impl<P, Container> TryToCv<cv::Mat> for image::ImageBuffer<P, Container>
where
    P: image::Pixel,
    P::Subpixel: OpenCvElement,
    Container: Deref<Target = [P::Subpixel]> + Clone,
{
    type Error = Error;

    fn try_to_cv(&self) -> Result<cv::Mat, Self::Error> {
        let (width, height) = self.dimensions();
        let cv_type = cv::CV_MAKETYPE(P::Subpixel::DEPTH, P::CHANNEL_COUNT as i32);
        // Create empty Mat and copy data
        let mut mat = cv::Mat::zeros(height as i32, width as i32, cv_type)?.to_mat()?;
        unsafe {
            let mat_data_ptr = mat.data_mut();
            let img_data = self.as_raw();
            let total_bytes = (width as usize * height as usize * P::CHANNEL_COUNT as usize) * std::mem::size_of::<P::Subpixel>();
            std::ptr::copy_nonoverlapping(
                img_data.as_ptr() as *const u8,
                mat_data_ptr,
                total_bytes
            );
        }
        Ok(mat)
    }
}

// &DynamicImage -> Mat
impl TryToCv<cv::Mat> for image::DynamicImage {
    type Error = Error;

    fn try_to_cv(&self) -> Result<cv::Mat, Self::Error> {
        use image::DynamicImage as D;

        let mat = match self {
            D::ImageLuma8(image) => image.try_to_cv()?,
            D::ImageLumaA8(image) => image.try_to_cv()?,
            D::ImageRgb8(image) => image.try_to_cv()?,
            D::ImageRgba8(image) => image.try_to_cv()?,
            D::ImageLuma16(image) => image.try_to_cv()?,
            D::ImageLumaA16(image) => image.try_to_cv()?,
            D::ImageRgb16(image) => image.try_to_cv()?,
            D::ImageRgba16(image) => image.try_to_cv()?,
            D::ImageRgb32F(image) => image.try_to_cv()?,
            D::ImageRgba32F(image) => image.try_to_cv()?,
            image => bail!("the color type {:?} is not supported", image.color()),
        };
        Ok(mat)
    }
}

// &Mat -> DynamicImage
impl TryToCv<image::DynamicImage> for cv::Mat {
    type Error = Error;

    fn try_to_cv(&self) -> Result<image::DynamicImage, Self::Error> {
        let rows = self.rows();
        let cols = self.cols();
        ensure!(
            rows != -1 && cols != -1,
            "Mat with more than 2 dimensions is not supported."
        );

        let depth = self.depth();
        let n_channels = self.channels();
        let width = cols as u32;
        let height = rows as u32;

        let image: image::DynamicImage = match (depth, n_channels) {
            (cv::CV_8U, 1) => mat_to_image_buffer_gray::<u8>(self, width, height).into(),
            (cv::CV_16U, 1) => mat_to_image_buffer_gray::<u16>(self, width, height).into(),
            (cv::CV_8U, 3) => mat_to_image_buffer_rgb::<u8>(self, width, height).into(),
            (cv::CV_16U, 3) => mat_to_image_buffer_rgb::<u16>(self, width, height).into(),
            (cv::CV_32F, 3) => mat_to_image_buffer_rgb::<f32>(self, width, height).into(),
            _ => bail!("Mat of type {} is not supported", self.type_name()),
        };

        Ok(image)
    }
}

// &Mat -> gray ImageBuffer
impl<T> TryToCv<image::ImageBuffer<image::Luma<T>, Vec<T>>> for cv::Mat
where
    image::Luma<T>: image::Pixel,
    T: OpenCvElement + image::Primitive + DataType,
{
    type Error = Error;

    fn try_to_cv(&self) -> Result<image::ImageBuffer<image::Luma<T>, Vec<T>>, Self::Error> {
        let rows = self.rows();
        let cols = self.cols();
        ensure!(
            rows != -1 && cols != -1,
            "Mat with more than 2 dimensions is not supported."
        );

        let depth = self.depth();
        let n_channels = self.channels();
        let width = cols as u32;
        let height = rows as u32;

        ensure!(
            n_channels == 1,
            "Unable to convert a multi-channel Mat to a gray image"
        );
        ensure!(depth == T::DEPTH, "Subpixel type is not supported");

        let image = mat_to_image_buffer_gray::<T>(self, width, height);
        Ok(image)
    }
}

// &Mat -> rgb ImageBuffer
impl<T> TryToCv<image::ImageBuffer<image::Rgb<T>, Vec<T>>> for cv::Mat
where
    image::Rgb<T>: image::Pixel<Subpixel = T>,
    T: OpenCvElement + image::Primitive + DataType,
{
    type Error = Error;

    fn try_to_cv(&self) -> Result<image::ImageBuffer<image::Rgb<T>, Vec<T>>, Self::Error> {
        let rows = self.rows();
        let cols = self.cols();
        ensure!(
            rows != -1 && cols != -1,
            "Mat with more than 2 dimensions is not supported."
        );

        let depth = self.depth();
        let n_channels = self.channels();
        let width = cols as u32;
        let height = rows as u32;

        ensure!(
            n_channels == 3,
            "Expect 3 channels, but get {n_channels} channels"
        );
        ensure!(depth == T::DEPTH, "Subpixel type is not supported");

        let image = mat_to_image_buffer_rgb::<T>(self, width, height);
        Ok(image)
    }
}

// Utility functions

fn mat_to_image_buffer_gray<T>(
    mat: &cv::Mat,
    width: u32,
    height: u32,
) -> image::ImageBuffer<image::Luma<T>, Vec<T>>
where
    T: image::Primitive + OpenCvElement + DataType,
{
    type Image<T> = image::ImageBuffer<image::Luma<T>, Vec<T>>;

    match mat.as_slice::<T>() {
        Ok(slice) => Image::<T>::from_vec(width, height, slice.to_vec()).unwrap(),
        Err(_) => Image::<T>::from_fn(width, height, |col, row| {
            let pixel: T = *mat.at_2d(row as i32, col as i32).unwrap();
            image::Luma([pixel])
        }),
    }
}

fn mat_to_image_buffer_rgb<T>(
    mat: &cv::Mat,
    width: u32,
    height: u32,
) -> image::ImageBuffer<image::Rgb<T>, Vec<T>>
where
    T: image::Primitive + OpenCvElement + DataType,
    image::Rgb<T>: image::Pixel<Subpixel = T>,
{
    type Image<T> = image::ImageBuffer<image::Rgb<T>, Vec<T>>;

    match mat.as_slice::<T>() {
        Ok(slice) => Image::<T>::from_vec(width, height, slice.to_vec()).unwrap(),
        Err(_) => Image::<T>::from_fn(width, height, |col, row| {
            let cv::Point3_::<T> { x, y, z } = *mat.at_2d(row as i32, col as i32).unwrap();
            image::Rgb([x, y, z])
        }),
    }
}

#[cfg(test)]
mod tests {
    use crate::image;
    use crate::opencv::{core as cv, prelude::*};
    use crate::with_opencv::MatExt;
    use crate::TryToCv;
    use anyhow::{ensure, Result};
    use itertools::iproduct;

    #[test]
    fn convert_opencv_image() -> Result<()> {
        const WIDTH: usize = 250;
        const HEIGHT: usize = 100;

        // gray
        {
            let mat = Mat::new_randn_2d(HEIGHT as i32, WIDTH as i32, cv::CV_8UC1)?;
            let image: image::GrayImage = mat.try_to_cv()?;
            let mat2: Mat = image.try_to_cv()?;

            iproduct!(0..HEIGHT, 0..WIDTH).try_for_each(|(row, col)| {
                let p1: u8 = *mat.at_2d(row as i32, col as i32)?;
                let p2 = image[(col as u32, row as u32)].0[0];
                let p3: u8 = *mat2.at_2d(row as i32, col as i32)?;
                ensure!(p1 == p2 && p1 == p3);
                anyhow::Ok(())
            })?;
        }

        // rgb
        {
            let mat = Mat::new_randn_2d(HEIGHT as i32, WIDTH as i32, cv::CV_8UC3)?;
            let image: image::RgbImage = mat.try_to_cv()?;
            let mat2: Mat = image.try_to_cv()?;

            iproduct!(0..HEIGHT, 0..WIDTH).try_for_each(|(row, col)| {
                let p1: cv::Point3_<u8> = *mat.at_2d(row as i32, col as i32)?;
                let p2: image::Rgb<u8> = image[(col as u32, row as u32)];
                let p3: cv::Point3_<u8> = *mat2.at_2d(row as i32, col as i32)?;
                ensure!(p1 == p3);
                ensure!({
                    let a1 = {
                        let cv::Point3_ { x, y, z } = p1;
                        [x, y, z]
                    };
                    let a2 = p2.0;
                    a1 == a2
                });
                anyhow::Ok(())
            })?;
        }

        Ok(())
    }
}
